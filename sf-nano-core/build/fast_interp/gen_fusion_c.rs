// Fused C handler code generator.
// Produces fast_fused_handlers.inc: StackSim, impl_* C handler bodies.

use super::op_classify::{
    get_pop_push, is_load_op, is_pure_binop, is_pure_unary, is_store_op, is_tos_none,
    is_trapping_binop, needs_ctx, CategoryMap,
};
use super::types::{FieldKind, FusedHandler};

/// Determine the IMPL_PARAMS_* macro name for a given pop/push pattern
fn impl_params_name(fused: &FusedHandler) -> &'static str {
    if is_tos_none(fused) {
        return "IMPL_PARAMS_NONE";
    }
    let (pop, push) = get_pop_push(fused);
    match (pop, push) {
        (0, 1) => "IMPL_PARAMS_POP0_PUSH1",
        (0, 2) => "IMPL_PARAMS_POP0_PUSH2",
        (0, 3) => "IMPL_PARAMS_POP0_PUSH3",
        (1, 0) => "IMPL_PARAMS_POP1_PUSH0",
        (1, 1) => "IMPL_PARAMS_POP1_PUSH1",
        (1, 2) => "IMPL_PARAMS_POP1_PUSH2",
        (2, 0) => "IMPL_PARAMS_POP2_PUSH0",
        (2, 1) => "IMPL_PARAMS_POP2_PUSH1",
        (2, 2) => "IMPL_PARAMS_POP2_PUSH2",
        _ => panic!(
            "No IMPL_PARAMS for pop={} push={} in handler {}",
            pop, push, fused.op
        ),
    }
}

/// Map base instruction to SEM_* macro expression for fused code generation.
/// Each base instruction is modeled as a stack operation:
///   - Result: what it pushes (or None for side-effect-only ops)
///   - Consumes: how many stack values it pops
struct StackSim {
    /// Named variables on the simulated stack
    vars: Vec<String>,
    /// Counter for generating unique variable names
    counter: usize,
    /// Lines of C code emitted
    lines: Vec<String>,
}

impl StackSim {
    fn new() -> Self {
        Self {
            vars: Vec::new(),
            counter: 0,
            lines: Vec::new(),
        }
    }

    fn fresh_var(&mut self) -> String {
        let name = format!("v{}_", self.counter);
        self.counter += 1;
        name
    }

    fn pop(&mut self) -> String {
        self.vars
            .pop()
            .expect("Stack underflow in fused handler simulation")
    }

    fn push(&mut self, var: String) {
        self.vars.push(var);
    }

    fn emit(&mut self, line: String) {
        self.lines.push(line);
    }

    /// Process one instruction in the fused pattern.
    /// Dispatches to the appropriate handler based on op category.
    fn process_op(
        &mut self,
        op: &str,
        field_name: Option<&str>,
        fused_op_name: &str,
        categories: &CategoryMap,
    ) {
        match op {
            // --- Special ops with immediates ---
            // General local ops — use SEM macros (fp[idx]).
            // At match time, l0 locals are excluded from these patterns,
            // so idx is guaranteed non-zero when l0 is active.
            "local_get" => {
                let fname = field_name.expect("local_get needs field name");
                let var = self.fresh_var();
                let decode = format!("{}_decode_{}(pc)", fused_op_name, fname);
                self.emit(format!("    uint16_t {} = {};", fname, decode));
                self.emit(format!(
                    "    uint64_t {} = SEM_LOCAL_GET(fp, {});",
                    var, fname
                ));
                self.push(var);
            }
            "local_set" => {
                let fname = field_name.expect("local_set needs field name");
                let val = self.pop();
                let decode = format!("{}_decode_{}(pc)", fused_op_name, fname);
                self.emit(format!("    uint16_t {} = {};", fname, decode));
                self.emit(format!("    SEM_LOCAL_SET(fp, {}, {});", fname, val));
            }
            "local_tee" => {
                let fname = field_name.expect("local_tee needs field name");
                let val = self.pop();
                let decode = format!("{}_decode_{}(pc)", fused_op_name, fname);
                self.emit(format!("    uint16_t {} = {};", fname, decode));
                self.emit(format!("    SEM_LOCAL_SET(fp, {}, {});", fname, val));
                self.push(val);
            }
            // L0 register ops — direct register access, no field decode needed.
            // The compiler can fully eliminate these as reg-to-reg copies when fused.
            "local_get_l0" => {
                let var = self.fresh_var();
                self.emit(format!("    uint64_t {} = *p_l0;", var));
                self.push(var);
            }
            "local_set_l0" => {
                let val = self.pop();
                self.emit(format!("    *p_l0 = (uint64_t)({});", val));
            }
            "local_tee_l0" => {
                let val = self.pop();
                self.emit(format!("    *p_l0 = (uint64_t)({});", val));
                self.push(val);
            }
            "i32_const" => {
                let fname = field_name.expect("i32_const needs field name");
                let var = self.fresh_var();
                let decode = format!("{}_decode_{}(pc)", fused_op_name, fname);
                self.emit(format!(
                    "    uint64_t {} = (uint64_t)(uint32_t){};",
                    var, decode
                ));
                self.push(var);
            }
            "i64_const" => {
                let fname = field_name.expect("i64_const needs field name");
                let var = self.fresh_var();
                let decode = format!("{}_decode_{}(pc)", fused_op_name, fname);
                self.emit(format!(
                    "    uint64_t {} = (uint64_t){};",
                    var, decode
                ));
                self.push(var);
            }
            "br_if" => {
                let cond = self.pop();
                self.emit(format!(
                    "    struct Instruction* target = (struct Instruction*){}_decode_target(pc);",
                    fused_op_name
                ));
                self.emit(format!("    if ((uint32_t){} != 0) {{", cond));
                self.emit("        return target;".to_string());
                self.emit("    }".to_string());
            }
            "if_" => {
                // IF semantics: condition == 0 jumps to else/end (target),
                // condition != 0 falls through to then-body (pc_next).
                // Uses guard-check dispatch, so the linear path (then) is fast.
                // Must use pattern-specific decode_target (not pc_alt) because
                // the target may not be in imm0 when other fields occupy it.
                let cond = self.pop();
                self.emit(format!(
                    "    struct Instruction* target = (struct Instruction*){}_decode_target(pc);",
                    fused_op_name
                ));
                self.emit(format!("    if ((uint32_t){} == 0) {{", cond));
                self.emit("        return target;".to_string());
                self.emit("    }".to_string());
            }

            // --- Load ops: pop addr, push value, trapping ---
            _ if is_load_op(categories, op) => {
                let fname = field_name.unwrap_or_else(|| panic!("{} needs field name", op));
                let addr = self.pop();
                let var = self.fresh_var();
                let decode = format!("{}_decode_{}(pc)", fused_op_name, fname);
                let sem = format!("SEM_{}", op.to_uppercase());
                self.emit(format!("    uint32_t {} = {};", fname, decode));
                self.emit(format!("    uint64_t {};", var));
                self.emit(format!("    {}(ctx, {}, {}, {});", sem, addr, fname, var));
                self.push(var);
            }

            // --- Store ops: pop addr + val, no push, trapping ---
            _ if is_store_op(categories, op) => {
                let fname = field_name.unwrap_or_else(|| panic!("{} needs field name", op));
                let val = self.pop();
                let addr = self.pop();
                let decode = format!("{}_decode_{}(pc)", fused_op_name, fname);
                let sem = format!("SEM_{}", op.to_uppercase());
                self.emit(format!("    uint32_t {} = {};", fname, decode));
                self.emit(format!("    {}(ctx, {}, {}, {});", sem, addr, fname, val));
            }

            // --- Trapping binops: pop 2, push 1, needs ctx ---
            _ if is_trapping_binop(categories, op) => {
                let sem = format!("SEM_{}", op.to_uppercase());
                let rhs = self.pop();
                let lhs = self.pop();
                let var = self.fresh_var();
                self.emit(format!("    uint64_t {};", var));
                self.emit(format!("    {}(ctx, {}, {}, {});", sem, lhs, rhs, var));
                self.push(var);
            }

            // --- Pure expression binops: pop 2, push 1 ---
            _ if is_pure_binop(categories, op) => {
                let sem = format!("SEM_{}", op.to_uppercase());
                let rhs = self.pop();
                let lhs = self.pop();
                let var = self.fresh_var();
                self.emit(format!("    uint64_t {} = {}({}, {});", var, sem, lhs, rhs));
                self.push(var);
            }

            // --- Pure expression unary: pop 1, push 1 ---
            _ if is_pure_unary(categories, op) => {
                let sem = format!("SEM_{}", op.to_uppercase());
                let val = self.pop();
                let var = self.fresh_var();
                self.emit(format!("    uint64_t {} = {}({});", var, sem, val));
                self.push(var);
            }

            _ => panic!("Unknown op in fused pattern: {}", op),
        }
    }
}

/// Generate fast_fused_handlers.inc content from fused handler definitions.
pub fn generate(fused_handlers: &[FusedHandler], categories: &CategoryMap) -> String {
    let mut code = String::new();

    code.push_str("// Auto-generated by gen_fusion.rs from handlers.toml\n");
    code.push_str("// DO NOT EDIT MANUALLY\n\n");
    code.push_str("#include <stdint.h>\n\n");
    code.push_str("#define fp (*pfp)\n\n");

    for fused in fused_handlers {
        generate_fused_c_handler(&mut code, fused, categories);
    }

    code.push_str("#undef fp\n");

    code
}

fn generate_fused_c_handler(code: &mut String, fused: &FusedHandler, categories: &CategoryMap) {
    let params_macro = impl_params_name(fused);
    let (pop, push) = get_pop_push(fused);
    let tos_none = is_tos_none(fused);
    let ctx_used = needs_ctx(categories, fused);
    let fields = fused.get_fields();

    code.push_str(&format!(
        "// {}: {}\n",
        fused.op,
        fused.pattern.join(" -> ")
    ));
    code.push_str(&format!(
        "FORCE_INLINE struct Instruction* impl_{}({}) {{\n",
        fused.op, params_macro
    ));

    // Suppress unused parameter warnings
    if !ctx_used {
        code.push_str("    (void)ctx;\n");
    }

    // Build a map from pattern index to field name (for ops that need field decode)
    let mut field_for_pattern: Vec<Option<&str>> = vec![None; fused.pattern.len()];
    for f in fields {
        if let Some(from_idx) = f.from {
            // For target fields, we handle them specially in br_if processing
            if f.kind == FieldKind::Target {
                // target is decoded directly in br_if
            } else {
                field_for_pattern[from_idx] = Some(&f.name);
            }
        }
    }

    // Simulate stack and emit code
    let mut sim = StackSim::new();

    // Pre-populate stack with TOS input values
    if !tos_none {
        match (pop, push) {
            (0, _) => {} // No inputs from stack
            (1, 0) => {
                sim.push("*p_src".to_string());
            }
            (1, 1) => {
                sim.push("*p_src".to_string());
            }
            (1, 2) => {
                sim.push("*p_src".to_string());
            }
            (2, 0) => {
                sim.push("*p_addr".to_string());
                sim.push("*p_val".to_string());
            }
            (2, 1) => {
                sim.push("*p_lhs".to_string());
                sim.push("*p_rhs".to_string());
            }
            (2, 2) => {
                sim.push("*p_lhs".to_string());
                sim.push("*p_rhs".to_string());
            }
            _ => panic!(
                "Unhandled pop/push pattern: pop={} push={} in {}",
                pop, push, fused.op
            ),
        }
    }

    // Process each pattern instruction
    for (i, op) in fused.pattern.iter().enumerate() {
        sim.process_op(op, field_for_pattern[i], &fused.op, categories);
    }

    // Emit the generated lines
    for line in &sim.lines {
        code.push_str(line);
        code.push('\n');
    }

    // Write outputs to TOS registers
    if !tos_none && push > 0 {
        // The remaining items on the simulated stack are our outputs
        // We need to assign them to the output pointers
        let outputs: Vec<String> = sim.vars.clone();
        match (pop, push) {
            (_, 1) if !tos_none => {
                if let Some(val) = outputs.last() {
                    code.push_str(&format!("    *p_dst = {};\n", val));
                }
            }
            (0, 2) | (1, 2) => {
                // p_dst0 = first output (deeper), p_dst1 = second (top)
                if outputs.len() >= 2 {
                    code.push_str(&format!("    *p_dst0 = {};\n", outputs[0]));
                    code.push_str(&format!("    *p_dst1 = {};\n", outputs[1]));
                }
            }
            (2, 2) => {
                // p_dst0 and p_dst1
                if outputs.len() >= 2 {
                    code.push_str(&format!("    *p_dst0 = {};\n", outputs[0]));
                    code.push_str(&format!("    *p_dst1 = {};\n", outputs[1]));
                }
            }
            (0, 3) => {
                if outputs.len() >= 3 {
                    code.push_str(&format!("    *p_dst0 = {};\n", outputs[0]));
                    code.push_str(&format!("    *p_dst1 = {};\n", outputs[1]));
                    code.push_str(&format!("    *p_dst2 = {};\n", outputs[2]));
                }
            }
            _ => {}
        }
    }

    code.push_str("    return pc_next(pc);\n");
    code.push_str("}\n\n");
}
