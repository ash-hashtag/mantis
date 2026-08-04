#[derive(Clone, Debug)]
pub struct ConstNode {}

#[derive(Clone, Debug)]
pub struct VarNode {}

#[derive(Clone, Debug)]
pub enum BinaryOperation {
    Add,
    Sub,
    Div,
    Mul,
    EqualTo,
    NotEqualTo,
    LessThan,
    GreaterThan,
    LogicAnd,
    LogicOr,
    BitwiseAnd,
    BitwiseOr,
    Cast,
    StructFieldAccess,
}

impl BinaryOperation {
    pub fn eval(&self, lhs: &Node, rhs: &Node) -> Node {
        use BinaryOperation::*;
        match self {
            Add => todo!(),
            Sub => todo!(),
            Div => todo!(),
            Mul => todo!(),
            EqualTo => todo!(),
            NotEqualTo => todo!(),
            LessThan => todo!(),
            GreaterThan => todo!(),
            LogicAnd => todo!(),
            LogicOr => todo!(),
            BitwiseAnd => todo!(),
            BitwiseOr => todo!(),
            Cast => todo!(),
            StructFieldAccess => todo!(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum Node {
    BinaryExpr {
        lhs: Box<Node>,
        rhs: Box<Node>,
        op: BinaryOperation,
    },
    Variable(VarNode),
    Constant(ConstNode),
    VarType(String),
    None,
}

impl Node {
    pub fn eval(&self) -> Node {
        match self {
            Node::BinaryExpr { lhs, rhs, op } => return op.eval(lhs, rhs),
            _ => {}
        }

        self.clone()
    }
}
