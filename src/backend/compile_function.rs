use crate::ms::MsContext;
use crate::native::instructions::NodeResult;
use crate::registries::functions::{FunctionType, MsDeclaredFunction};
use crate::registries::modules::MsResolved;
use crate::registries::structs::MsEnumType;
use crate::registries::types::{
    binary_cmp_op_to_condcode_intcc, MsNativeType, MsType, MsTypeId, MsTypeWithId,
};
use crate::registries::variable::{MsVal, MsVar};
use crate::registries::MsRegistryExt;
use crate::scope::{drop_scope, drop_variable};
use codegen::ir::Inst;
use cranelift::{codegen::Context, prelude::*};
use cranelift_module::{DataDescription, Linkage, Module};
use cranelift_object::ObjectModule;
use linear_map::LinearMap;
use logos::Source;
use mantis_parser::ast::{
    BinOp as BinaryOperation, Block, BlockItem, ConditionalBlock, Expr, FieldInit, FnDecl,
    Ident as WordSpan, IfChain as IfElseChain, LoopBlock, Statement, TypeExpr as MsTokenType,
    UnaryOp,
};
use mantis_parser::token::Span;
use rand::Rng;
use std::fmt::Write;
use std::ops::Deref;
use std::rc::Rc;

pub enum FinalType {
    Name(Box<str>),
    Type(Type),
    CraneliftType(types::Type),
}

pub struct MethodFor<'a> {
    pub trait_name: Option<&'a str>,
    pub on_type: &'a MsTypeWithId,
}

pub fn compile_function(
    function: FnDecl,
    module: &mut ObjectModule,
    ctx: &mut Context,
    fbx: &mut FunctionBuilderContext,
    ms_ctx: &mut MsContext,
    trait_on_type: Option<MethodFor>,
    exporting_fn_name: Option<&str>,
) -> Rc<MsDeclaredFunction> {
    let name = function
        .name
        .as_ref()
        .map(|n| match n {
            MsTokenType::Named(id) => &id.name,
            MsTokenType::Generic(base, _) => match &**base {
                MsTokenType::Named(id) => &id.name,
                _ => panic!("unhandled function name type"),
            },
            _ => panic!("unhandled function name type"),
        })
        .expect("function must have a name");
    let mut linkage = Linkage::Preemptible;
    if function.is_extern {
        if function.body.is_none() {
            linkage = Linkage::Import;
        } else {
            linkage = Linkage::Export;
        }
    }

    let self_type = if let Some(trait_on_type) = &trait_on_type {
        let ty = trait_on_type.on_type.clone();
        Some(ty)
    } else {
        None
    };

    let mut returns_struct_or_enum = false;
    let return_ty = {
        if let Some(ref ret_type) = function.return_type {
            if !matches!(ret_type, MsTokenType::Unknown) {
                let ty = ms_ctx
                    .current_module
                    .resolve(ret_type)
                    .expect(&format!("invalid type_name"))
                    .ty()
                    .unwrap();

                match ty.ty {
                    MsType::Native(nty) => {
                        ctx.func.signature.returns.push(nty.to_abi_param().unwrap());
                    }
                    MsType::Struct(struct_ty) => {
                        returns_struct_or_enum = true;
                        ctx.func.signature.params.push(struct_ty.to_abi_param());
                    }
                    MsType::Enum(enum_ty) => {
                        returns_struct_or_enum = true;
                        ctx.func.signature.params.push(enum_ty.to_abi_param());
                    }
                    _ => todo!(),
                };

                Some(ty.id)
            } else {
                None
            }
        } else {
            None
        }
    };

    let mut fn_arguments = LinearMap::<Box<str>, MsTypeId>::with_capacity(function.params.len());
    for param in &function.params {
        let ty = ms_ctx
            .current_module
            .resolve(&param.ty)
            .unwrap()
            .ty()
            .unwrap();

        ctx.func
            .signature
            .params
            .push(ty.ty.to_abi_param().unwrap());

        fn_arguments.insert(param.name.name.as_str().into(), ty.id);
    }

    let func_id = module
        .declare_function(
            exporting_fn_name.unwrap_or(name),
            linkage,
            &ctx.func.signature,
        )
        .unwrap();

    let declared_function = Rc::new(MsDeclaredFunction {
        func_id,
        arguments: fn_arguments,
        rets: return_ty,
        fn_type: if function.is_extern {
            FunctionType::Extern
        } else {
            FunctionType::Public
        },
    });
    ms_ctx
        .current_module
        .fn_registry
        .add_function(exporting_fn_name.unwrap_or(name), declared_function.clone());

    if let Some(tot) = &trait_on_type {
        if let Some(trait_name) = tot.trait_name {
            ms_ctx.current_module.trait_registry.add_function(
                trait_name,
                tot.on_type.id.clone(),
                name.as_str().into(),
                declared_function.clone(),
            );
        }

        ms_ctx.current_module.type_fn_registry.add_function(
            tot.on_type.id,
            name.as_str(),
            declared_function.clone(),
        );
    }

    if function.body.is_none() {
        ctx.clear();
        return declared_function;
    }
    let mut f = FunctionBuilder::new(&mut ctx.func, fbx);
    ms_ctx.var_scopes.new_scope();

    let entry_block = f.create_block();
    f.append_block_params_for_function_params(entry_block);
    f.switch_to_block(entry_block);

    let block_params = f.block_params(entry_block).to_vec();

    let mut fn_args_iter = block_params.iter();
    if returns_struct_or_enum {
        let return_ty_id = return_ty.unwrap();
        let ty = ms_ctx
            .current_module
            .type_registry
            .get_from_type_id(return_ty_id)
            .expect(&format!("invalid type_name"));
        let var = f.declare_var(ty.to_cl_type().unwrap());
        let value = fn_args_iter.next().unwrap();
        f.def_var(var, *value);
        ms_ctx
            .var_scopes
            .add_variable("return", MsVar::new(return_ty_id, var, None, true, true));
    }

    for ((arg_name, arg_type), value) in declared_function.arguments.iter().zip(fn_args_iter) {
        let ty = ms_ctx
            .current_module
            .type_registry
            .get_from_type_id(arg_type.clone())
            .expect("invalid type_name");
        let var = f.declare_var(ty.to_cl_type().unwrap());
        f.def_var(var, *value);
        ms_ctx.var_scopes.add_variable(
            arg_name.deref(),
            MsVar::new(arg_type.clone(), var, None, true, true), // ignoring mutability
        );
    }

    {
        let body = function.body.as_ref().unwrap();
        if compile_block(body, module, &mut f, ms_ctx).is_none() {
            let scope = ms_ctx.var_scopes.exit_scope().unwrap();
            drop_scope(&scope, ms_ctx, &mut f, module);
            f.ins().return_(&[]);
        }
    }
    f.seal_block(entry_block);

    f.finalize();
    module.define_function(func_id, ctx).unwrap();
    ctx.clear();

    return declared_function;
}

pub fn compile_block(
    block: &Block,
    module: &mut ObjectModule,
    fbx: &mut FunctionBuilder,
    ms_ctx: &mut MsContext,
) -> Option<Inst> {
    let mut last_inst = None;
    for item in &block.items {
        match item {
            BlockItem::Statement(stmt) => {
                let stmts = std::slice::from_ref(stmt);
                if let Some(inst) = compile_statements(stmts, module, fbx, ms_ctx) {
                    return Some(inst);
                }
            }
            BlockItem::IfChain(if_chain) => {
                compile_if_else_chain(if_chain, fbx, module, ms_ctx);
            }
            BlockItem::Loop(loop_block) => {
                compile_loop(loop_block, module, fbx, ms_ctx);
            }
            BlockItem::Match(_match_block) => todo!("match cases unimplemented"),
            BlockItem::Block(inner_block) => {
                ms_ctx.var_scopes.new_scope();
                compile_block(inner_block, module, fbx, ms_ctx);
                let vars = ms_ctx.var_scopes.exit_scope().unwrap();
                drop_scope(&vars, ms_ctx, fbx, module);
            }
        }
    }

    return last_inst;
}

pub fn compile_binary_operation(
    op: BinaryOperation,
    lhs: Value,
    rhs: Value,
    ty: MsNativeType,
    module: &mut ObjectModule,
    fbx: &mut FunctionBuilder,
    ms_ctx: &mut MsContext,
) -> Value {
    match op {
        BinaryOperation::Add => ty.add(lhs, rhs, fbx),
        BinaryOperation::Sub => ty.sub(lhs, rhs, fbx),
        BinaryOperation::Div => ty.div(lhs, rhs, fbx),
        BinaryOperation::Mul => ty.mult(lhs, rhs, fbx),
        BinaryOperation::Mod => {
            if ty.is_uint() {
                fbx.ins().urem(lhs, rhs)
            } else {
                fbx.ins().srem(lhs, rhs)
            }
        }
        BinaryOperation::Gt
        | BinaryOperation::GtEq
        | BinaryOperation::Eq
        | BinaryOperation::NotEq
        | BinaryOperation::Lt
        | BinaryOperation::LtEq => ty.compare(op, lhs, rhs, fbx),
        BinaryOperation::Shl => fbx.ins().ishl(lhs, rhs),
        BinaryOperation::Shr => {
            if ty.is_uint() {
                fbx.ins().ushr(lhs, rhs)
            } else {
                fbx.ins().sshr(lhs, rhs)
            }
        }
        BinaryOperation::BitAnd => fbx.ins().band(lhs, rhs),
        BinaryOperation::BitOr => fbx.ins().bor(lhs, rhs),
        BinaryOperation::BitXor => fbx.ins().bxor(lhs, rhs),
        _ => unimplemented!("unhandled binary operation {:?}", op),
    }
}

pub fn compile_assignment(
    lhs: &str,
    rhs: NodeResult,
    module: &mut ObjectModule,
    fbx: &mut FunctionBuilder,
    ms_ctx: &mut MsContext,
) {
    let variable = ms_ctx
        .var_scopes
        .find_variable(lhs)
        .expect(&format!("undeclared variable {lhs}"));

    if !variable.is_mutable {
        panic!("{lhs} is marked immutable");
    }

    assert!(
        variable.ty_id == rhs.ty(),
        "{} are not equal types {}",
        variable.ty_id,
        rhs.ty()
    );

    let ty = ms_ctx
        .current_module
        .type_registry
        .get_from_type_id(variable.ty_id)
        .unwrap();
    match ty {
        MsType::Native(nty) => {
            let value = rhs.value(fbx, ms_ctx);
            fbx.def_var(variable.c_var, value);
            if let Some(ss) = variable.stack_slot {
                let ptr = fbx.ins().stack_addr(types::I64, ss, 0);
                fbx.ins().store(MemFlags::new(), value, ptr, 0);
            }
        }
        MsType::Struct(sty) => {
            // TODO: drop the old struct
            let mut dropping_var = variable.clone();
            dropping_var.is_reference = false;
            drop_variable(&dropping_var, ms_ctx, fbx, module);
            let dest = variable.value(fbx, ms_ctx);
            let src = rhs.value(fbx, ms_ctx);
            sty.copy(dest, src, fbx, module, ms_ctx);

            if let Some(ss) = variable.stack_slot {
                let dest_ptr = fbx.ins().stack_addr(types::I64, ss, 0);
                sty.copy(dest_ptr, src, fbx, module, ms_ctx);
            }
        }
        _ => todo!(),
    }
}

pub fn compile_assignment_on_pointers(
    lhs: NodeResult,
    rhs: NodeResult,
    module: &mut ObjectModule,
    fbx: &mut FunctionBuilder,
    ms_ctx: &mut MsContext,
) {
    let ty = ms_ctx
        .current_module
        .type_registry
        .get_from_type_id(rhs.ty())
        .unwrap();
    match ty {
        MsType::Native(nty) => {
            let value = rhs.value(fbx, ms_ctx);
            let ptr = lhs.value(fbx, ms_ctx);
            fbx.ins().store(MemFlags::new(), value, ptr, 0);
        }
        MsType::Struct(sty) => {
            let dest = lhs.value(fbx, ms_ctx);
            let src = rhs.value(fbx, ms_ctx);
            sty.copy(dest, src, fbx, module, ms_ctx);
        }
        MsType::Enum(ety) => {
            let dest = lhs.value(fbx, ms_ctx);
            let src = rhs.value(fbx, ms_ctx);
            ety.copy(dest, src, fbx, module, ms_ctx);
        }
        _ => todo!(),
    }
}

pub fn compile_cast(
    value: NodeResult,
    cast_to: MsTypeWithId,

    module: &mut ObjectModule,
    fbx: &mut FunctionBuilder,
    ms_ctx: &mut MsContext,
) -> NodeResult {
    let ty = ms_ctx
        .current_module
        .type_registry
        .get_from_type_id(value.ty())
        .unwrap();

    match ty {
        MsType::Native(nty) => {
            return NodeResult::Val(MsVal::new(
                cast_to.id,
                nty.cast_to(value.value(fbx, ms_ctx), &cast_to.ty, fbx),
            ))
        }
        _ => unimplemented!("only native types are castable"),
    }
}

pub fn compile_nested_struct_field_to_ptr(
    root_struct_ptr: Value,
    root_struct_ty: MsTypeId,
    child: &MsTokenType,
    ms_ctx: &mut MsContext,
    fbx: &mut FunctionBuilder,
) -> NodeResult {
    let mut root_ty = ms_ctx
        .current_module
        .type_registry
        .get_from_type_id(root_struct_ty)
        .expect(&format!("didn't find type {}", root_struct_ty));

    if let MsType::Ref(inner, _) = root_ty {
        root_ty = *inner;
    }

    let MsType::Struct(struct_ty) = root_ty else {
        panic!("expected struct, found {:?}", root_struct_ty);
    };

    match child {
        MsTokenType::Named(ident) => {
            let field_name = ident.name.as_str();
            let ty = struct_ty;
            let field = ty.get_field(field_name).expect(&format!(
                "no field with name {} on {:?}",
                field_name, root_struct_ty
            ));
            let value = fbx.ins().iadd_imm(root_struct_ptr, field.offset as i64);
            return NodeResult::Val(MsVal::new(field.ty, value));
        }
        MsTokenType::Nested(parent_ty, child_ty) => {
            let ty = struct_ty;
            let field_name = parent_ty.as_name().unwrap();
            let field = ty.get_field(field_name).expect(&format!(
                "no field with name {} on {:?}",
                field_name, root_struct_ty
            ));
            let value = fbx.ins().iadd_imm(root_struct_ptr, field.offset as i64);
            return compile_nested_struct_field_to_ptr(value, field.ty, child_ty, ms_ctx, fbx);
        }
        _ => unreachable!(),
    }

    todo!()
}

pub fn compile_node(
    node: &Expr,
    module: &mut ObjectModule,
    fbx: &mut FunctionBuilder,
    ms_ctx: &mut MsContext,
) -> Option<NodeResult> {
    match node {
        Expr::Cast { expr, ty, span } => {
            let value = compile_node(expr, module, fbx, ms_ctx).unwrap();
            let ty_name = ty.as_name().unwrap();
            let ty = ms_ctx
                .current_module
                .type_registry
                .get_from_str(ty_name)
                .expect(&format!("undefined type {}", ty_name));
            let value = compile_cast(value, ty.clone(), module, fbx, ms_ctx);
            return Some(value);
        }
        Expr::PointerAssign {
            target,
            value,
            span,
        } => {
            let rhs = compile_node(value, module, fbx, ms_ctx).unwrap();
            let lhs = compile_node(target, module, fbx, ms_ctx).unwrap();
            compile_assignment_on_pointers(lhs, rhs, module, fbx, ms_ctx);
            return None;
        }
        Expr::Binary { op, lhs, rhs, span } => {
            if matches!(op, BinaryOperation::Assign) {
                log::info!("Assignment Operation {:?} = {:?}", lhs, rhs);
                match lhs.as_ref() {
                    Expr::Ident(ident) => {
                        let variable_name = ident.name.as_str();
                        let rhs = compile_node(rhs, module, fbx, ms_ctx).unwrap();
                        compile_assignment(variable_name, rhs, module, fbx, ms_ctx);
                    }
                    Expr::Field { object, field, .. } => {
                        // Nested struct field assignment
                        let type_expr = field_access_to_type_expr(lhs);
                        if let MsTokenType::Nested(root, child) = &type_expr {
                            let variable_name = root.as_name().unwrap();
                            let var = ms_ctx.var_scopes.find_variable(variable_name).unwrap();
                            let ptr = var.value(fbx, ms_ctx);
                            let final_ptr_res = compile_nested_struct_field_to_ptr(
                                ptr, var.ty_id, child, ms_ctx, fbx,
                            );
                            let final_ptr = final_ptr_res.value(fbx, ms_ctx);
                            let ty = ms_ctx
                                .current_module
                                .type_registry
                                .get_from_type_id(final_ptr_res.ty())
                                .unwrap();
                            match ty {
                                MsType::Struct(_) | MsType::Enum(_) | MsType::Native(_) => {
                                    let rhs = compile_node(rhs, module, fbx, ms_ctx).unwrap();
                                    compile_assignment_on_pointers(
                                        final_ptr_res,
                                        rhs,
                                        module,
                                        fbx,
                                        ms_ctx,
                                    );
                                }
                                _ => unreachable!("{:?}", ty),
                            }
                        } else if let MsTokenType::Named(ident) = &type_expr {
                            let variable_name = ident.name.as_str();
                            let rhs_val = compile_node(rhs, module, fbx, ms_ctx).unwrap();
                            compile_assignment(variable_name, rhs_val, module, fbx, ms_ctx);
                        }
                    }
                    Expr::Call { callee, args, .. } => {
                        let lhs_result = compile_node(lhs, module, fbx, ms_ctx).unwrap();
                        let callee_type_expr = expr_to_type_expr(callee);
                        if let MsType::Enum(_lhs_ty) = ms_ctx
                            .current_module
                            .type_registry
                            .get_from_type_id(lhs_result.ty())
                            .unwrap()
                        {
                            if let Some((enum_ty, variant)) =
                                check_if_its_enum_unwrap(&callee_type_expr, ms_ctx)
                            {
                                let rhs = compile_node(rhs, module, fbx, ms_ctx).unwrap();
                                let enum_ptr = rhs.value(fbx, ms_ctx);
                                let enum_inner = match args.first().unwrap() {
                                    Expr::Ident(id) => id.name.as_str(),
                                    _ => unreachable!(),
                                };

                                let res_var = fbx.declare_var(types::I64);
                                let inner_ptr = enum_ty.get_inner_ptr(rhs.value(fbx, ms_ctx), fbx);
                                fbx.def_var(res_var, inner_ptr);
                                let inner_ty = enum_ty.get_inner_ty(&variant).unwrap();
                                let var = MsVar::new(inner_ty.id, res_var, None, true, true);
                                ms_ctx.var_scopes.add_variable(enum_inner, var);
                                let op =
                                    binary_cmp_op_to_condcode_intcc(BinaryOperation::Eq, false);

                                let tag = enum_ty.get_tag(enum_ptr, fbx);
                                let expected_tag = enum_ty.get_tag_index(&variant).unwrap();
                                let value = fbx.ins().icmp_imm(op, tag, expected_tag as i64);

                                let ty = ms_ctx
                                    .current_module
                                    .resolve_from_str("bool")
                                    .unwrap()
                                    .ty()
                                    .unwrap();
                                return Some(NodeResult::Val(MsVal::new(ty.id, value)));
                            } else {
                                panic!("Can't assign to this lvalue");
                            }
                        } else {
                            panic!("Can't assign to this lvalue");
                        }
                    }
                    _ => {
                        let rhs = compile_node(rhs, module, fbx, ms_ctx).unwrap();
                        let lhs = compile_node(lhs, module, fbx, ms_ctx).unwrap();
                        compile_assignment_on_pointers(lhs, rhs, module, fbx, ms_ctx);
                    }
                };
                return None;
            } else {
                let lhs_result = compile_node(lhs, module, fbx, ms_ctx).unwrap();
                let rhs_result = compile_node(rhs, module, fbx, ms_ctx).unwrap();

                let ty = ms_ctx
                    .current_module
                    .type_registry
                    .get_from_type_id(lhs_result.ty())
                    .unwrap();

                match ty {
                    MsType::Native(nty) => {
                        let lval = lhs_result.value(fbx, ms_ctx);
                        let rval = rhs_result.value(fbx, ms_ctx);
                        let value =
                            compile_binary_operation(*op, lval, rval, nty, module, fbx, ms_ctx);
                        let mut res_ty = lhs_result.ty();
                        if matches!(
                            op,
                            BinaryOperation::Gt
                                | BinaryOperation::GtEq
                                | BinaryOperation::Eq
                                | BinaryOperation::NotEq
                                | BinaryOperation::Lt
                                | BinaryOperation::LtEq
                        ) {
                            res_ty = ms_ctx
                                .current_module
                                .resolve_from_str("bool")
                                .unwrap()
                                .ty()
                                .unwrap()
                                .id;
                        }
                        return Some(NodeResult::Val(MsVal::new(res_ty, value)));
                    }
                    MsType::Struct(_sty) => {
                        panic!("structs can't be used with == operator");
                    }
                    MsType::Enum(enum_ty) => {
                        let tag = enum_ty.get_tag(rhs_result.value(fbx, ms_ctx), fbx);
                        let expected_tag = lhs_result.value(fbx, ms_ctx);
                        let nty = MsNativeType::Bool;
                        let value = compile_binary_operation(
                            *op,
                            tag,
                            expected_tag,
                            nty,
                            module,
                            fbx,
                            ms_ctx,
                        );
                        let bool_ty = ms_ctx
                            .current_module
                            .resolve_from_str("bool")
                            .unwrap()
                            .ty()
                            .unwrap();
                        return Some(NodeResult::Val(MsVal::new(bool_ty.id, value)));
                    }
                    _ => unreachable!(),
                };
            }
        }
        Expr::Unary { op, operand, span } => match op {
            UnaryOp::Deref => {
                let node = compile_node(operand, module, fbx, ms_ctx).unwrap();
                let ptr_value = node.value(fbx, ms_ctx);
                let ty = ms_ctx
                    .current_module
                    .type_registry
                    .get_from_type_id(node.ty())
                    .unwrap();
                match ty.clone() {
                    MsType::Native(nty) => {
                        let val = fbx.ins().load(
                            nty.to_cl_type().unwrap(),
                            MemFlags::new(),
                            ptr_value,
                            0,
                        );
                        return Some(NodeResult::Val(MsVal::new(node.ty(), val)));
                    }
                    MsType::Struct(_struct_ty) => {
                        return Some(NodeResult::Val(MsVal::new(node.ty(), ptr_value)));
                    }
                    MsType::Ref(inner, _) => match inner.as_ref() {
                        MsType::Native(nty) => {
                            let val = fbx.ins().load(
                                nty.to_cl_type().unwrap(),
                                MemFlags::new(),
                                ptr_value,
                                0,
                            );
                            let inner_ty_id = ms_ctx
                                .current_module
                                .type_registry
                                .add_type(random_string(20), inner.as_ref().clone());
                            return Some(NodeResult::Val(MsVal::new(inner_ty_id, val)));
                        }
                        _ => {
                            let inner_ty_id = ms_ctx
                                .current_module
                                .type_registry
                                .add_type(random_string(20), inner.as_ref().clone());
                            return Some(NodeResult::Val(MsVal::new(inner_ty_id, ptr_value)));
                        }
                    },
                    _ => todo!(),
                };
            }
            UnaryOp::Neg => {
                let node = compile_node(operand, module, fbx, ms_ctx).unwrap();
                let val = node.value(fbx, ms_ctx);
                let neg_val = fbx.ins().ineg(val);
                return Some(NodeResult::Val(MsVal::new(node.ty(), neg_val)));
            }
            UnaryOp::AddrOf => match operand.as_ref() {
                Expr::Ident(ident) => {
                    let var = ms_ctx.var_scopes.find_variable(&ident.name).unwrap();
                    let ptr = if let Some(ss) = var.stack_slot {
                        fbx.ins().stack_addr(types::I64, ss, 0)
                    } else {
                        var.value(fbx, ms_ctx)
                    };
                    let ty = ms_ctx
                        .current_module
                        .type_registry
                        .get_from_type_id(var.ty_id)
                        .unwrap();
                    let ref_ty = MsType::Ref(Box::new(ty.clone()), var.is_mutable);
                    let ref_ty_id = ms_ctx
                        .current_module
                        .type_registry
                        .add_type(random_string(20), ref_ty);
                    return Some(NodeResult::Val(MsVal::new(ref_ty_id, ptr)));
                }
                _ => todo!("address-of only implemented for identifiers"),
            },
        },
        Expr::Call { callee, args, span } => {
            let fn_type_expr = expr_to_type_expr(callee);

            let mut method_on_variable: Option<MsVar> = None;
            let func = match &fn_type_expr {
                MsTokenType::Nested(root, child) => {
                    let either_var_or_ty_name = root.as_name().unwrap();
                    if let Some(var) = ms_ctx
                        .var_scopes
                        .find_variable(either_var_or_ty_name)
                        .cloned()
                    {
                        let method_name = child.as_name().unwrap();
                        let reg = ms_ctx
                            .current_module
                            .type_fn_registry
                            .map
                            .get(&var.ty_id)
                            .unwrap();
                        let func = reg.registry.get(method_name).unwrap();
                        method_on_variable = Some(var.clone());
                        func.clone()
                    } else {
                        let Some(MsResolved::Function(func)) =
                            ms_ctx.current_module.resolve(&fn_type_expr)
                        else {
                            panic!("couldn't find function {:?}", fn_type_expr);
                        };
                        func
                    }
                }
                _ => match ms_ctx.current_module.resolve(&fn_type_expr).unwrap() {
                    MsResolved::Function(func) => func,
                    MsResolved::EnumUnwrap(enum_ty, variant_name) => {
                        let MsType::Enum(enum_ty) = enum_ty.ty else {
                            unreachable!()
                        };
                        assert!(args.len() == 1);
                        let variant_type = enum_ty.get_inner_ty(&variant_name).unwrap();
                        let arg = args.first().unwrap();
                        if let Expr::Ident(var_ident) = arg {
                            let res_var = fbx.declare_var(variant_type.ty.to_cl_type().unwrap());
                            todo!("either move this enum unwrapping to assignment handling");
                        } else {
                            unreachable!();
                        }
                        return None;
                    }
                    _ => unreachable!(),
                },
            };

            let mut call_arg_values = Vec::with_capacity(args.len());
            if let Some(fn_ret_ty) = func.rets {
                let return_ty = ms_ctx
                    .current_module
                    .type_registry
                    .get_from_type_id(fn_ret_ty)
                    .unwrap();

                let mut returns_a_struct_ptr: Option<Value> = None;
                match return_ty {
                    MsType::Native(_nty) => {}
                    MsType::Struct(sty) => {
                        let stackslot = fbx.create_sized_stack_slot(StackSlotData::new(
                            StackSlotKind::ExplicitSlot,
                            sty.size() as u32,
                            0,
                        ));
                        let ptr = fbx.ins().stack_addr(types::I64, stackslot, 0);
                        call_arg_values.push(ptr);
                        returns_a_struct_ptr = Some(ptr);
                    }
                    MsType::Enum(ety) => {
                        let stackslot = fbx.create_sized_stack_slot(StackSlotData::new(
                            StackSlotKind::ExplicitSlot,
                            ety.size() as u32,
                            0,
                        ));
                        let ptr = fbx.ins().stack_addr(types::I64, stackslot, 0);
                        call_arg_values.push(ptr);
                        returns_a_struct_ptr = Some(ptr);
                    }
                    _ => todo!(),
                }

                let mut arg_idx = 0;
                if let Some(var) = method_on_variable {
                    call_arg_values.push(var.value(fbx, ms_ctx));
                    arg_idx = 1;
                }

                for arg in args {
                    let arg_val = compile_node(arg, module, fbx, ms_ctx).unwrap();
                    let mut val = arg_val.value(fbx, ms_ctx);

                    // Auto-unwrap StrSlice to pointer if i64 is expected
                    if let Some(&expected_ty_id) = func.arguments.values().nth(arg_idx) {
                        let str_slice_ty = ms_ctx
                            .current_module
                            .type_registry
                            .get_from_str("StrSlice")
                            .map(|t| t.id);
                        let i64_ty_id = ms_ctx
                            .current_module
                            .type_registry
                            .get_from_str("i64")
                            .map(|t| t.id);

                        if Some(arg_val.ty()) == str_slice_ty && Some(expected_ty_id) == i64_ty_id {
                            let ty = ms_ctx
                                .current_module
                                .type_registry
                                .get_from_type_id(arg_val.ty())
                                .unwrap();
                            if let MsType::Struct(sty) = ty {
                                let field = sty.get_field("pointer").unwrap();
                                let field_addr = fbx.ins().iadd_imm(val, field.offset as i64);
                                val = fbx.ins().load(types::I64, MemFlags::new(), field_addr, 0);
                            }
                        }
                    }

                    call_arg_values.push(val);
                    arg_idx += 1;
                }

                let func_ref = module.declare_func_in_func(func.func_id, fbx.func);
                log::info!("calling a function {:?}", callee);
                let inst = fbx.ins().call(func_ref, &call_arg_values);
                let result = fbx.inst_results(inst);

                if !result.is_empty() {
                    let return_value = result[0];
                    return Some(NodeResult::Val(MsVal::new(fn_ret_ty, return_value)));
                } else if let Some(ptr) = returns_a_struct_ptr {
                    return Some(NodeResult::Val(MsVal::new(fn_ret_ty, ptr)));
                }
            } else {
                let mut arg_idx = 0;
                if let Some(var) = method_on_variable {
                    call_arg_values.push(var.value(fbx, ms_ctx));
                    arg_idx = 1;
                }

                for arg in args {
                    let arg_val = compile_node(arg, module, fbx, ms_ctx).unwrap();
                    let mut val = arg_val.value(fbx, ms_ctx);

                    // Auto-unwrap StrSlice to pointer if i64 is expected
                    if let Some(&expected_ty_id) = func.arguments.values().nth(arg_idx) {
                        let str_slice_ty = ms_ctx
                            .current_module
                            .type_registry
                            .get_from_str("StrSlice")
                            .map(|t| t.id);
                        let i64_ty_id = ms_ctx
                            .current_module
                            .type_registry
                            .get_from_str("i64")
                            .map(|t| t.id);

                        if Some(arg_val.ty()) == str_slice_ty && Some(expected_ty_id) == i64_ty_id {
                            let ty = ms_ctx
                                .current_module
                                .type_registry
                                .get_from_type_id(arg_val.ty())
                                .unwrap();
                            if let MsType::Struct(sty) = ty {
                                let field = sty.get_field("pointer").unwrap();
                                let field_addr = fbx.ins().iadd_imm(val, field.offset as i64);
                                val = fbx.ins().load(types::I64, MemFlags::new(), field_addr, 0);
                            }
                        }
                    }

                    call_arg_values.push(val);
                    arg_idx += 1;
                }

                let func_ref = module.declare_func_in_func(func.func_id, fbx.func);
                log::info!("calling a function {:?}", callee);
                let _inst = fbx.ins().call(func_ref, &call_arg_values);
            }
        }
        Expr::Ident(ident) => {
            let var_name = ident.name.as_str();
            if let Some(var) = ms_ctx.var_scopes.find_variable(var_name) {
                return Some(NodeResult::Var(var.clone()));
            } else {
                panic!("undefined {} word or type name in current scope", var_name,);
            }
        }
        Expr::Field {
            object,
            field,
            span: _,
        } => {
            let obj_node = compile_node(object, module, fbx, ms_ctx)?;
            let child = MsTokenType::Named(field.clone());
            return Some(compile_nested_struct_access(
                obj_node, &child, ms_ctx, fbx, module,
            ));
        }
        Expr::IntLit { value, span } => {
            let ty = ms_ctx
                .current_module
                .type_registry
                .get_from_str("i64")
                .unwrap();
            let cty = ty.ty.to_cl_type().unwrap();
            let val = fbx.ins().iconst(cty, *value);
            return Some(NodeResult::Val(MsVal::new(ty.id, val)));
        }
        Expr::FloatLit { value, span } => {
            let ty = ms_ctx
                .current_module
                .type_registry
                .get_from_str("f64")
                .unwrap();
            let cty = ty.ty.to_cl_type().unwrap();
            let val = fbx.ins().f64const(*value);
            return Some(NodeResult::Val(MsVal::new(ty.id, val)));
        }
        Expr::CharLit { value, span } => {
            let c = *value as i32;
            let ty = ms_ctx
                .current_module
                .type_registry
                .get_from_str("char")
                .unwrap();
            let cty = ty.ty.to_cl_type().unwrap();
            let val = fbx.ins().iconst(cty, c as i64);
            return Some(NodeResult::Val(MsVal::new(ty.id, val)));
        }
        Expr::BoolLit { value, span } => {
            let ty = ms_ctx
                .current_module
                .resolve_from_str("bool")
                .unwrap()
                .ty()
                .unwrap();
            let val = fbx.ins().iconst(types::I8, *value as i64);
            return Some(NodeResult::Val(MsVal::new(ty.id, val)));
        }
        Expr::StringLit { value, span } => {
            let content = value.clone();
            let mut data_name = String::with_capacity(32);
            {
                random_string_into(20, &mut data_name);
                write!(&mut data_name, "_{}_{}", span.start, span.end).unwrap();
            }

            let ty = ms_ctx
                .current_module
                .type_registry
                .get_from_str("StrSlice")
                .unwrap();
            let MsType::Struct(sty) = ty.ty else {
                panic!("expected struct");
            };
            let stack_slot = fbx.create_sized_stack_slot(StackSlotData::new(
                StackSlotKind::ExplicitSlot,
                sty.size() as u32,
                0,
            ));
            let size_of_data = fbx.ins().iconst(types::I64, content.len() as i64);
            let ty_i64 = ms_ctx
                .current_module
                .type_registry
                .get_from_str("i64")
                .unwrap();
            let struct_ptr = fbx.ins().stack_addr(types::I64, stack_slot, 0);

            let data_ptr = if content.is_empty() {
                fbx.ins().iconst(ty_i64.ty.to_cl_type().unwrap(), 0)
            } else {
                let data_id = module
                    .declare_data(&data_name, Linkage::Preemptible, false, false)
                    .unwrap();
                let mut data_desc = DataDescription::new();
                let mut bytes = content.as_bytes().to_vec();
                bytes.push(0);
                data_desc.define(bytes.into());
                module.define_data(data_id, &data_desc).unwrap();
                let gl_value = module.declare_data_in_func(data_id, fbx.func);
                fbx.ins()
                    .global_value(ty_i64.ty.to_cl_type().unwrap(), gl_value)
            };

            sty.set_field(
                &MsVal::new(ty_i64.id, struct_ptr),
                "len",
                &MsVal::new(ty_i64.id, size_of_data),
                ms_ctx,
                fbx,
                module,
            );
            sty.set_field(
                &MsVal::new(ty_i64.id, struct_ptr),
                "pointer",
                &MsVal::new(ty_i64.id, data_ptr),
                ms_ctx,
                fbx,
                module,
            );

            return Some(NodeResult::Val(MsVal::new(ty.id, struct_ptr)));
        }
        Expr::StructInit { ty, fields, span } => {
            let ty_name = ty.as_name().unwrap();
            let ty = ms_ctx
                .current_module
                .type_registry
                .get_from_str(ty_name)
                .expect(&format!("couldn't find type_name {ty_name}"));

            let MsType::Struct(struct_type) = ty.ty else {
                panic!("undefined struct {}", ty_name);
            };

            let stack_slot = fbx.create_sized_stack_slot(StackSlotData::new(
                StackSlotKind::ExplicitSlot,
                struct_type.size() as u32,
                0,
            ));
            let ptr = fbx.ins().stack_addr(types::I64, stack_slot, 0);
            let ptr = MsVal::new(ty.id, ptr);

            for field_init in fields {
                let field_name = field_init.name.name.as_str();
                let val = compile_node(&field_init.value, module, fbx, ms_ctx);
                let val = val.unwrap();
                let val = val.to_ms_val(fbx, ms_ctx);
                struct_type.set_field(&ptr, field_name, &val, ms_ctx, fbx, module);
            }

            return Some(NodeResult::Val(ptr));
        }
        Expr::ArrayInit { elements, span } => {
            let mut arr_ty: Option<MsTypeId> = None;
            let mut nodes = Vec::new();
            for element in elements {
                let compiled = compile_node(element, module, fbx, ms_ctx).unwrap();
                if let Some(ty) = arr_ty.clone() {
                    assert!(ty == compiled.ty());
                } else {
                    arr_ty = Some(compiled.ty());
                }
                nodes.push(compiled);
            }
            let arr_inner_ty = ms_ctx
                .current_module
                .type_registry
                .get_from_type_id(arr_ty.unwrap())
                .unwrap();
            let arr_stack_size = arr_inner_ty.size() * nodes.len() + 8;
            let _stackslot = fbx.create_sized_stack_slot(StackSlotData::new(
                StackSlotKind::ExplicitSlot,
                arr_stack_size as u32,
                0,
            ));
            todo!("array init");
        }
        Expr::Lambda { decl, span } => todo!("lambda compilation"),
        Expr::CompilerCall { name, args, span } => {
            if name == "init" {
                let ty_expr = if let Expr::TypeExpr(ty) = args.first().unwrap() {
                    ty
                } else if let Expr::Ident(id) = args.first().unwrap() {
                    // This is a bit of a hack because sometimes the parser might give an Ident
                    &MsTokenType::Named(id.clone())
                } else {
                    panic!("Expected type for #init");
                };

                let resolved = ms_ctx
                    .current_module
                    .resolve(ty_expr)
                    .expect("undefined type")
                    .ty()
                    .unwrap();
                let size = resolved.ty.size();

                let malloc_func = ms_ctx
                    .current_module
                    .fn_registry
                    .registry
                    .get("malloc")
                    .expect("malloc function not defined");
                let malloc_ref = module.declare_func_in_func(malloc_func.func_id, fbx.func);
                let size_val = fbx.ins().iconst(types::I64, size as i64);
                let call = fbx.ins().call(malloc_ref, &[size_val]);
                let res = fbx.inst_results(call)[0];

                let ptr_ty = MsType::Ref(Box::new(resolved.ty.clone()), true);
                let ptr_ty_id = ms_ctx.current_module.type_registry.add_type(random_string(20), ptr_ty);

                return Some(NodeResult::Val(MsVal::new(ptr_ty_id, res)));
            }
            if name == "size_of" {
                let Some(arg) = args.first() else {
                    panic!("#size_of requires an argument");
                };
                let resolved = match arg {
                    Expr::TypeExpr(ty) => ms_ctx.current_module.resolve(ty),
                    Expr::Ident(id) => ms_ctx.current_module.resolve_from_str(&id.name),
                    _ => panic!("Expected type for #size_of"),
                }
                .expect("undefined type")
                .ty()
                .expect("not a type");

                let i64_ty = ms_ctx
                    .current_module
                    .type_registry
                    .get_from_str("i64")
                    .unwrap();
                let value = fbx.ins().iconst(types::I64, resolved.ty.size() as i64);
                return Some(NodeResult::Val(MsVal::new(i64_ty.id, value)));
            }
            if name == "as_ref" || name == "ref" {
                let arg = args.first().expect(&format!("#{} requires an argument", name));
                let res = compile_node(arg, module, fbx, ms_ctx)
                    .expect(&format!("failed to compile argument for #{}", name));
                let inner_ty_id = res.ty();
                let inner_ty = ms_ctx
                    .current_module
                    .type_registry
                    .get_from_type_id(inner_ty_id)
                    .unwrap();

                let ref_ty = match inner_ty {
                    MsType::Ref(inner, _) => MsType::Ref(inner, false),
                    _ => MsType::Ref(Box::new(inner_ty.clone()), false),
                };
                let ref_ty_id = ms_ctx
                    .current_module
                    .type_registry
                    .add_type(random_string(20), ref_ty);

                return Some(NodeResult::Val(MsVal::new(ref_ty_id, res.value(fbx, ms_ctx))));
            }
            if name == "ptr" {
                let arg = args.first().expect("#ptr requires an argument");
                let res = compile_node(arg, module, fbx, ms_ctx)
                    .expect("failed to compile argument for #ptr");
                let inner_ty_id = res.ty();
                let inner_ty = ms_ctx
                    .current_module
                    .type_registry
                    .get_from_type_id(inner_ty_id)
                    .unwrap();

                let ptr_ty = match inner_ty {
                    MsType::Ref(inner, _) => MsType::Ref(inner, true),
                    _ => MsType::Ref(Box::new(inner_ty.clone()), true),
                };
                let ptr_ty_id = ms_ctx
                    .current_module
                    .type_registry
                    .add_type(random_string(20), ptr_ty);

                return Some(NodeResult::Val(MsVal::new(ptr_ty_id, res.value(fbx, ms_ctx))));
            }
            if name == "free" {
                let arg = args.first().expect("#free requires an argument");
                let res = compile_node(arg, module, fbx, ms_ctx)
                    .expect("failed to compile argument for #free");
                let free_func = ms_ctx
                    .current_module
                    .fn_registry
                    .registry
                    .get("free")
                    .expect("free function not defined");
                let free_ref = module.declare_func_in_func(free_func.func_id, fbx.func);
                let ptr = res.value(fbx, ms_ctx);
                fbx.ins().call(free_ref, &[ptr]);
                return None;
            }
            todo!("compiler call {}", name);
        }
        Expr::TypeExpr(_) => todo!("type expr in expression position"),
        Expr::Propagate { expr, span } => todo!("propagate operator"),
    }

    return None;
}

/// Convert a field access Expr chain (e.g. `a.b.c`) to a TypeExpr (Nested) chain
fn field_access_to_type_expr(expr: &Expr) -> MsTokenType {
    match expr {
        Expr::Ident(id) => MsTokenType::Named(id.clone()),
        Expr::Field { object, field, .. } => {
            let root = field_access_to_type_expr(object);
            let child = MsTokenType::Named(field.clone());
            MsTokenType::Nested(Box::new(root), Box::new(child))
        }
        _ => MsTokenType::Unknown,
    }
}

/// Convert an expression to a TypeExpr for module resolution (function names, type paths, etc.)
fn expr_to_type_expr(expr: &Expr) -> MsTokenType {
    field_access_to_type_expr(expr)
}

pub fn compile_nested_struct_access(
    root: NodeResult,
    child: &MsTokenType,
    ms_ctx: &mut MsContext,
    fbx: &mut FunctionBuilder,
    module: &mut ObjectModule,
) -> NodeResult {
    // let var_name = root.word().unwrap();
    // if let Some(var) = ms_ctx.var_scopes.find_variable(&var_name).cloned() {

    let var = root;
    let var_ty = ms_ctx
        .current_module
        .type_registry
        .get_from_type_id(var.ty())
        .unwrap();

    match var_ty.clone() {
        MsType::Native(ms_native_type) => {
            return var;
        }
        MsType::Ref(inner, _) => {
            // Automatically dereference if it's a reference to a struct
            if let MsType::Struct(struct_ty) = inner.as_ref() {
                let inner_ty_id = ms_ctx
                    .current_module
                    .type_registry
                    .add_type(random_string(20), MsType::Struct(struct_ty.clone()));
                let ptr = var.value(fbx, ms_ctx);
                let root_val = NodeResult::Val(MsVal::new(inner_ty_id, ptr));
                return compile_nested_struct_access(root_val, child, ms_ctx, fbx, module);
            }
            return var;
        }
        MsType::Struct(struct_ty) => {
            match child {
                MsTokenType::Named(ident) => {
                    let field_name = ident.name.as_str();
                    let field = struct_ty
                        .get_field(field_name)
                        .expect(&format!("unknown field in struct {}", field_name));

                    let offset = field.offset;
                    let target_ty_id = field.ty;

                    match var {
                        NodeResult::Var(v) => {
                            if let Some(ss) = v.stack_slot {
                                let ptr = fbx.ins().stack_addr(types::I64, ss, 0);
                                return NodeResult::StructAccessVar {
                                    ptr,
                                    offset,
                                    ty_id: target_ty_id,
                                };
                            }
                            return NodeResult::StructAccessVar {
                                ptr: v.value(fbx, ms_ctx),
                                offset,
                                ty_id: target_ty_id,
                            };
                        }
                        NodeResult::Val(v) => {
                            return NodeResult::StructAccessVar {
                                ptr: v.value,
                                offset,
                                ty_id: target_ty_id,
                            };
                        }
                        NodeResult::StructAccessVar {
                            ptr,
                            offset: old_offset,
                            ..
                        } => {
                            return NodeResult::StructAccessVar {
                                ptr,
                                offset: old_offset + offset,
                                ty_id: target_ty_id,
                            };
                        }
                    }
                }
                MsTokenType::Nested(parent_ty, child_ty) => {
                    let field_name = parent_ty.as_name().unwrap();
                    let field = struct_ty
                        .get_field(field_name)
                        .expect(&format!("unknown field in struct {}", field_name));

                    let offset = field.offset;
                    let target_ty_id = field.ty;

                    let root_ptr = match var {
                        NodeResult::Var(v) => {
                            if let Some(ss) = v.stack_slot {
                                fbx.ins().stack_addr(types::I64, ss, 0)
                            } else {
                                v.value(fbx, ms_ctx)
                            }
                        }
                        NodeResult::Val(v) => v.value,
                        NodeResult::StructAccessVar {
                            ptr,
                            offset: old_offset,
                            ..
                        } => fbx.ins().iadd_imm(ptr, old_offset as i64),
                    };

                    let field_ptr = fbx.ins().iadd_imm(root_ptr, offset as i64);
                    let field_node = NodeResult::Val(MsVal::new(target_ty_id, field_ptr));
                    return compile_nested_struct_access(field_node, child_ty, ms_ctx, fbx, module);
                }
                _ => unreachable!(),
            }
        }
        MsType::Enum(enum_ty) => {
            let variant_name = child.as_name().unwrap();
            let tag_idx = enum_ty.get_tag_index(variant_name).unwrap();
            let i64_ty = ms_ctx
                .current_module
                .resolve_from_str("i64")
                .unwrap()
                .ty()
                .unwrap();

            let value = fbx
                .ins()
                .iconst(i64_ty.ty.to_cl_type().unwrap(), tag_idx as i64);
            return NodeResult::Val(MsVal::new(i64_ty.id, value));
        }
        _ => todo!(),
    };
}

pub fn implicit_cast(
    var: NodeResult,
    ty: MsTypeWithId,
    _module: &mut ObjectModule,
    fbx: &mut FunctionBuilder,
    ms_ctx: &mut MsContext,
) -> NodeResult {
    if var.ty() == ty.id {
        return var;
    }

    let actual_ty = ms_ctx
        .current_module
        .type_registry
        .get_from_type_id(var.ty())
        .unwrap();

    if actual_ty == ty.ty {
        return NodeResult::Val(MsVal::new(ty.id, var.value(fbx, ms_ctx)));
    }

    // Allow casting between different reference/pointer types if they are physically the same
    match (&actual_ty, &ty.ty) {
        (MsType::Ref(..), MsType::Ref(..)) => {
            return NodeResult::Val(MsVal::new(ty.id, var.value(fbx, ms_ctx)));
        }
        _ => {}
    }

    panic!(
        "implicit cast failed: couldn't cast {:?} to {:?}",
        actual_ty, ty.ty
    );
}

pub fn compile_statements(
    statements: &[Statement],
    module: &mut ObjectModule,
    fbx: &mut FunctionBuilder,
    ms_ctx: &mut MsContext,
) -> Option<Inst> {
    for stmt in statements {
        match stmt {
            Statement::Let {
                mutable,
                name,
                ty: expected_type,
                value: ref init_expr,
                span: _,
            } => {
                let var_name = name.name.as_str();
                let mut value = compile_node(init_expr, module, fbx, ms_ctx).unwrap();

                if let Some(expected_ty) = expected_type {
                    if !matches!(expected_ty, MsTokenType::Unknown) {
                        let resolved = ms_ctx
                            .current_module
                            .resolve(expected_ty)
                            .expect("undefined type");
                        let ty = match resolved {
                            MsResolved::Type(ty) => ty,
                            MsResolved::TypeRef(inner, is_mut) => {
                                let ms_type = MsType::Ref(Box::new(inner.ty.clone()), is_mut);
                                let id = ms_ctx
                                    .current_module
                                    .type_registry
                                    .add_type(random_string(20), ms_type.clone());
                                MsTypeWithId { id, ty: ms_type }
                            }
                            _ => panic!("expected a type"),
                        };
                        value = implicit_cast(value, ty, module, fbx, ms_ctx);
                    }
                }

                let node_value = value;
                let ty = node_value.ty();

                let ty = ms_ctx
                    .current_module
                    .type_registry
                    .get_from_type_id(ty)
                    .unwrap();

                let value = node_value.value(fbx, ms_ctx);
                let cl_ty = ty.to_cl_type().unwrap();
                let variable = fbx.declare_var(cl_ty);
                fbx.def_var(variable, value);

                let stack_slot = fbx.create_sized_stack_slot(StackSlotData::new(
                    StackSlotKind::ExplicitSlot,
                    ty.size() as u32,
                    0,
                ));
                let ptr = fbx.ins().stack_addr(types::I64, stack_slot, 0);
                fbx.ins().store(MemFlags::new(), value, ptr, 0);

                let mut variable = MsVar::new(
                    node_value.ty(),
                    variable,
                    Some(stack_slot),
                    *mutable,
                    false,
                );
                if let Some(old_variable) = ms_ctx.var_scopes.add_variable(var_name, variable) {
                    drop_variable(&old_variable, ms_ctx, fbx, module);
                }
            }
            Statement::Return {
                value: ref node,
                span: _,
            } => {
                let ret_inst = if let Some(ref expr) = node {
                    if let Some(var) = compile_node(expr, module, fbx, ms_ctx) {
                        let ty = ms_ctx
                            .current_module
                            .type_registry
                            .get_from_type_id(var.ty())
                            .unwrap();

                        match ty {
                            MsType::Native(nty) => {
                                let mut val = var.value(fbx, ms_ctx);
                                if !fbx.func.signature.returns.is_empty() {
                                    let expected_ty = fbx.func.signature.returns[0].value_type;
                                    let actual_ty = fbx.func.dfg.value_type(val);
                                    if expected_ty == types::I32 && actual_ty == types::I64 {
                                        val = fbx.ins().ireduce(types::I32, val);
                                    } else if expected_ty == types::I64 && actual_ty == types::I32 {
                                        val = fbx.ins().uextend(types::I64, val);
                                    }
                                }
                                if fbx.func.signature.returns.is_empty() {
                                    fbx.ins().return_(&[])
                                } else {
                                    fbx.ins().return_(&[val])
                                }
                            }
                            MsType::Struct(sty) => {
                                let src = var.value(fbx, ms_ctx);
                                let dest = ms_ctx.var_scopes.find_variable("return").unwrap().c_var;
                                let dest = fbx.use_var(dest);
                                sty.copy(dest, src, fbx, module, ms_ctx);
                                fbx.ins().return_(&[])
                            }
                            MsType::Ref(_, _) => {
                                let val = var.value(fbx, ms_ctx);
                                if !fbx.func.signature.returns.is_empty() {
                                    fbx.ins().return_(&[val])
                                } else {
                                    fbx.ins().return_(&[])
                                }
                            }
                            _ => todo!(),
                        }
                    } else {
                        fbx.ins().return_(&[])
                    }
                } else {
                    fbx.ins().return_(&[])
                };

                return Some(ret_inst);
            }
            Statement::Break { label, span: _ } => {
                let name = label.as_ref().map(|x| x.name.as_str());
                return Some(ms_ctx.loop_scopes.break_out_of_loop(name, fbx));
            }
            Statement::Continue { label, span: _ } => {
                let name = label.as_ref().map(|x| x.name.as_str());
                return Some(ms_ctx.loop_scopes.continue_loop(name, fbx));
            }
            Statement::Expr {
                expr: ref node,
                span: _,
            } => {
                compile_node(node, module, fbx, ms_ctx);
            }
        }
    }

    return None;
}

pub fn compile_loop(
    loop_block: &LoopBlock,
    module: &mut ObjectModule,
    fbx: &mut FunctionBuilder,
    ms_ctx: &mut MsContext,
) {
    let loop_name = loop_block.label.as_ref().map(|x| x.name.as_str());
    ms_ctx.var_scopes.new_scope();
    ms_ctx.new_loop_scope(loop_name.map(|x| x.into()), fbx);

    compile_block(&loop_block.body, module, fbx, ms_ctx);

    ms_ctx.loop_scopes.end_loop(loop_name, fbx);
    let scope = ms_ctx.var_scopes.exit_scope().unwrap();
    drop_scope(&scope, &ms_ctx, fbx, module);
}

pub struct IfElseChainBuilder {
    else_block: Option<cranelift::prelude::Block>,
    end_block: cranelift::prelude::Block,
}

impl IfElseChainBuilder {
    pub fn new_block(
        block: &ConditionalBlock,
        fbx: &mut FunctionBuilder,
        module: &mut ObjectModule,
        ms_ctx: &mut MsContext,
    ) -> Self {
        let if_block = fbx.create_block();
        let else_block = fbx.create_block();
        let end_block = fbx.create_block();

        ms_ctx.var_scopes.new_scope();

        let value = compile_node(&block.condition, module, fbx, ms_ctx).unwrap();
        let value = value.value(fbx, ms_ctx);
        fbx.ins().brif(value, if_block, &[], else_block, &[]);
        fbx.switch_to_block(if_block);

        let inst = compile_block(&block.body, module, fbx, ms_ctx);
        let scope = ms_ctx.var_scopes.exit_scope().unwrap();
        drop_scope(&scope, ms_ctx, fbx, module);
        if inst.is_none() {
            fbx.ins().jump(end_block, &[]);
        }

        fbx.seal_block(if_block);

        Self {
            else_block: Some(else_block),
            end_block,
        }
    }

    pub fn elseif_block(
        &mut self,
        block: &ConditionalBlock,
        fbx: &mut FunctionBuilder,
        module: &mut ObjectModule,
        ms_ctx: &mut MsContext,
    ) {
        let if_block = fbx.create_block();
        let else_block = fbx.create_block();
        let previous_else_block = self.else_block.replace(else_block).unwrap();
        let end_block = self.end_block;
        fbx.switch_to_block(previous_else_block);

        ms_ctx.var_scopes.new_scope();

        let value = compile_node(&block.condition, module, fbx, ms_ctx).unwrap();
        let value = value.value(fbx, ms_ctx);
        fbx.ins().brif(value, if_block, &[], else_block, &[]);
        fbx.seal_block(previous_else_block);

        fbx.switch_to_block(if_block);

        let inst = compile_block(&block.body, module, fbx, ms_ctx);
        let scope = ms_ctx.var_scopes.exit_scope().unwrap();
        drop_scope(&scope, ms_ctx, fbx, module);
        if inst.is_none() {
            fbx.ins().jump(end_block, &[]);
        }
        fbx.seal_block(if_block);
    }

    pub fn else_block(
        &mut self,
        block: &Block,
        fbx: &mut FunctionBuilder,
        module: &mut ObjectModule,
        ms_ctx: &mut MsContext,
    ) {
        let else_block = self.else_block.take().unwrap();
        fbx.switch_to_block(else_block);
        ms_ctx.var_scopes.new_scope();
        let inst = compile_block(&block, module, fbx, ms_ctx);
        let scope = ms_ctx.var_scopes.exit_scope().unwrap();
        drop_scope(&scope, ms_ctx, fbx, module);
        if inst.is_none() {
            fbx.ins().jump(self.end_block, &[]);
        }
        fbx.seal_block(else_block);
    }

    pub fn end(
        &mut self,
        fbx: &mut FunctionBuilder,
        module: &mut ObjectModule,
        ms_ctx: &mut MsContext,
    ) {
        if self.else_block.is_some() {
            let empty_block = Block {
                items: vec![],
                span: Span { start: 0, end: 0 },
            };
            self.else_block(&empty_block, fbx, module, ms_ctx);
        }
        fbx.switch_to_block(self.end_block);
        fbx.seal_block(self.end_block);
    }
}

pub fn compile_if_else_chain(
    if_else_chain: &IfElseChain,
    fbx: &mut FunctionBuilder,
    module: &mut ObjectModule,
    ms_ctx: &mut MsContext,
) {
    let mut builder = IfElseChainBuilder::new_block(&if_else_chain.if_block, fbx, module, ms_ctx);

    for elseifblocks in if_else_chain.elif_blocks.iter() {
        builder.elseif_block(&elseifblocks, fbx, module, ms_ctx);
    }

    if let Some(elseblock) = &if_else_chain.else_block {
        builder.else_block(&elseblock, fbx, module, ms_ctx);
    }

    builder.end(fbx, module, ms_ctx);
}

use std::sync::atomic::{AtomicUsize, Ordering};
static COUNTER: AtomicUsize = AtomicUsize::new(0);

pub fn random_string(len: usize) -> String {
    let count = COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("blk_{}", count)
}

pub fn random_string_into(len: usize, mut w: impl Write) {
    let count = COUNTER.fetch_add(1, Ordering::SeqCst);
    w.write_fmt(format_args!("blk_{}", count)).unwrap();
}

pub fn check_if_its_enum_unwrap(
    type_name: &MsTokenType,
    ms_ctx: &mut MsContext,
) -> Option<(Rc<MsEnumType>, Box<str>)> {
    let resolved = ms_ctx.current_module.resolve(type_name)?;
    return match resolved {
        MsResolved::EnumUnwrap(enum_ty, variant_name) => {
            Some((enum_ty.ty.enum_ty().unwrap(), variant_name))
        }
        _ => None,
    };
}
