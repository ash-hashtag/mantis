use std::collections::HashMap;

pub mod compiler_functions;
pub mod functions;
pub mod modules;
pub mod structs;
pub mod traits;
pub mod types;
pub mod variable;

pub trait MsRegistry<T> {
    fn get_registry(&self) -> &HashMap<String, T>;
    fn get_registry_mut(&mut self) -> &mut HashMap<String, T>;
}

pub trait MsRegistryExt<T>
where
    Self: MsRegistry<T>,
{
    fn get(&self, s: &str) -> Option<&T> {
        self.get_registry().get(s)
    }

    fn add(&mut self, k: String, v: T) -> Option<()> {
        if self.get_registry_mut().insert(k, v).is_none() {
            Some(())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;
    use linear_map::LinearMap;
    use cranelift_module::FuncId;
    use super::types::{MsTypeNameRegistry, MsType, MsNativeType, MsTypeMethodRegistry, MsTypeId};
    use super::traits::{MsTraitRegistry, MsTrait, MsTraitMethod};
    use super::functions::{MsFunctionRegistry, MsDeclaredFunction, FunctionType};

    fn mock_function(id: u32) -> Rc<MsDeclaredFunction> {
        Rc::new(MsDeclaredFunction {
            arguments: LinearMap::new(),
            rets: None,
            fn_type: FunctionType::Public,
            func_id: FuncId::from_u32(id),
        })
    }

    #[test]
    fn test_type_registry_resolver() {
        let mut registry = MsTypeNameRegistry::with_default_types();
        let i32_ty = registry.get_from_str("i32").expect("i32 should be in default types");
        assert_eq!(i32_ty.ty, MsType::Native(MsNativeType::I32));
    }

    #[test]
    fn test_trait_registry() {
        let mut trait_registry = MsTraitRegistry::default();
        let trait_name: Box<str> = "Display".into();
        
        let mut methods = std::collections::HashMap::new();
        methods.insert("fmt".into(), MsTraitMethod {
            name: "fmt".into(),
            args: vec![],
            ret: None,
        });

        let ms_trait = MsTrait {
            name: trait_name.clone(),
            methods,
        };

        trait_registry.add_trait(trait_name.clone(), ms_trait);

        let resolved_trait = trait_registry.get_trait("Display").unwrap();
        assert_eq!(resolved_trait.name, trait_name);
        assert!(resolved_trait.methods.contains_key("fmt"));
    }

    #[test]
    fn test_method_registry_at_type_level() {
        let mut method_registry = MsTypeMethodRegistry::default();
        let type_id = MsTypeId(1); // Mock TypeId
        let func = mock_function(100);
        
        method_registry.add_method(type_id, "hello", func.clone());
        
        let resolved_func = method_registry.get_method(type_id, "hello").unwrap();
        assert_eq!(resolved_func.func_id, func.func_id);
    }

    #[test]
    fn test_trait_implementation() {
        let mut trait_registry = MsTraitRegistry::default();
        let type_id = MsTypeId(1);
        let trait_name: Box<str> = "Display".into();
        
        let mut fn_registry = MsFunctionRegistry::default();
        let func = mock_function(200);
        fn_registry.add_function("fmt", func.clone());

        trait_registry.add_implementation(type_id, trait_name.clone(), fn_registry);

        let impl_func = trait_registry.find_method_implementation(type_id, "Display", "fmt").unwrap();
        assert_eq!(impl_func.func_id, func.func_id);
    }
}
