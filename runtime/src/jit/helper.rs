use cranelift::codegen::ir::{AbiParam, FuncRef, Function, Type, types};
use cranelift::module::{Linkage, Module, FuncId};
use cranelift::jit::JITModule;

// Stores the global FuncIds
pub struct HelperIds {
    pub get_global: FuncId,
    pub call: FuncId,
    pub load_string: FuncId,
}

impl HelperIds {
    pub fn new(module: &mut JITModule) -> Self {
        Self {
            get_global: Self::declare(module, "vm_get_global", &[types::I64, types::I64, types::I64, types::I8]),
            call: Self::declare(module, "vm_call", &[types::I64, types::I8, types::I8, types::I8]),
            load_string: Self::declare(module, "vm_load_string", &[types::I64, types::I64, types::I64, types::I8]),
        }
    }

    fn declare(module: &mut JITModule, name: &str, params: &[Type]) -> FuncId {
        let mut sig = module.make_signature();
        for &p in params {
            sig.params.push(AbiParam::new(p));
        }
        module.declare_function(name, Linkage::Import, &sig).expect("Failed to declare helper")
    }
}

// Stores the local FuncRefs
pub struct HelperRefs {
    pub get_global: FuncRef,
    pub call: FuncRef,
    pub load_string: FuncRef,
}

impl HelperRefs {
    pub fn new(ids: &HelperIds, module: &mut JITModule, func: &mut Function) -> Self {
        Self {
            get_global: module.declare_func_in_func(ids.get_global, func),
            call: module.declare_func_in_func(ids.call, func),
            load_string: module.declare_func_in_func(ids.load_string, func),
        }
    }
}