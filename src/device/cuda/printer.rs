//! Provides functions to print PTX code.
use device::cuda::{Gpu, Namer};
use codegen::*;
use ir::{self, dim, op, Operand, Size, Type};
use itertools::Itertools;
use search_space::{DimKind, Domain, InstFlag};
use std::io::Write;
use std::fmt::Write as WriteFmt;
use self::InstArg::*;
use utils::*;
// TODO(cc_perf): avoid concatenating strings.

/// Prints a rounding mode selector.
fn rounding(rounding: op::Rounding) -> &'static str {
    match rounding {
        op::Rounding::Exact => "",
        op::Rounding::Nearest => ".rn",
        op::Rounding::Zero => ".rz",
        op::Rounding::Positive => ".rp",
        op::Rounding::Negative => ".rm",
    }
}

/// Prints a `MulMode` selector.
fn mul_mode(from: Type, to: Type, fun: &Function) -> &'static str {
    match (lower_type(from, fun), lower_type(to, fun)) {
        (Type::I(32), Type::I(64)) => ".wide",
        (ref x, ref y) if x == y => ".lo",
        _ => panic!(),
    }
}

/// Prints a load operator.
fn ld_operator(flag: InstFlag) -> &'static str {
    match flag {
        InstFlag::MEM_SHARED => "ld.shared",
        InstFlag::MEM_CA => "ld.global.ca",
        InstFlag::MEM_CG => "ld.global.cg",
        InstFlag::MEM_CS => "ld.global.cs",
        InstFlag::MEM_NC => "ld.global.nc",
        _ => panic!("invalid load flag {:?}", flag),
    }
}

/// Prints a store operator.
fn st_operator(flag: InstFlag) -> &'static str {
    match flag {
        InstFlag::MEM_SHARED => "st.shared",
        InstFlag::MEM_CA => "st.global.wb",
        InstFlag::MEM_CG => "st.global.cg",
        InstFlag::MEM_CS => "st.global.cs",
        _ => panic!("invalid store flag {:?}", flag),
    }
}

/// Prints a `Type`.
fn ptx_type(t: Type) -> String {
    match t {
        Type::Void => panic!("void type cannot be printed"),
        Type::I(1) => "pred".to_string(),
        Type::I(size) => format!("s{size}", size = size),
        Type::F(size) => format!("f{size}", size = size),
        _ => panic!()
    }
}

/// Prints a `Type` for the host.
fn host_type(t: &Type) -> &'static str {
    match *t {
        Type::Void => "void",
        Type::PtrTo(..) => "CUdeviceptr",
        Type::F(32) => "float",
        Type::F(64) => "double",
        Type::I(8) => "int8_t",
        Type::I(16) => "int16_t",
        Type::I(32) => "int32_t",
        Type::I(64) => "int64_t",
        ref t => panic!("invalid type for the host: {}", t)
    }
}

/// Prints a parameter decalartion.
fn param_decl(param: &ParamVal, namer: &NameMap) -> String {
    format!(
        ".param .{t}{attr} {name}",
        t = ptx_type(param.t()),
        attr = if param.is_pointer() { ".ptr.global.align 16" } else { "" },
        name = namer.name_param(param.key()),
    )
}

/// Represents an argument of a PTX instruction.
enum InstArg<'a> {
    Inst(&'a Instruction<'a>),
    Op(&'a Operand<'a>),
    Addr(&'a Operand<'a>),
    VecInst(&'a Instruction<'a>, dim::Id, u32),
    VecOp(&'a Operand<'a>, dim::Id, u32),
}

/// Prints an instruction argument.
fn inst_arg(arg: &InstArg, namer: &mut NameMap) -> String {
    match *arg {
        Inst(inst) => namer.name_inst(inst).to_string(),
        Op(op) => namer.name_op(op).to_string(),
        Addr(op) => format!("[{}]", namer.name_op(op)),
        VecInst(inst, dim, size) => {
            let names = (0..size).map(|i| {
                namer.indexed_inst_name(inst, dim, i).to_string()
            }).collect_vec().join(", ");
            format!("{{{names}}}", names = names)
        },
        VecOp(op, dim, size) => {
            let names = (0..size).map(|i| {
                namer.indexed_op_name(op, dim, i).to_string()
            }).collect_vec().join(", ");
            format!("{{{names}}}", names = names)
        }
    }
}

/// Assembles the different parts of an instruction in a single string.
fn assemble(operator: &str,
            predicate: Option<RcStr>,
            t: Type,
            args: &[InstArg],
            namer: &mut NameMap) -> String {
    let args_str = args.iter().map(|x| inst_arg(x, namer)).collect_vec().join(", ");
    let pred = if let Some(ref pred) = predicate {
        format!("@{} ", pred)
    } else { String::new() };
    format!("{}{}.{} {};", pred, operator, ptx_type(t), args_str)
}

// TODO(cleanup): remove this function once values are preprocessed by codegen. If values
// are preprocessed, types will be already lowered.
fn lower_type(t: ir::Type, fun: &Function) -> ir::Type {
    unwrap!(fun.space().ir_instance().device().lower_type(t, fun.space()))
}

/// Returns the string representing a binary operator.
fn binary_op(op: ir::BinOp) -> &'static str {
    match op {
        ir::BinOp::Add => "add",
        ir::BinOp::Sub => "sub",
        ir::BinOp::Div => "div",
    }
}

/// Prints an instruction.
fn inst(inst: &Instruction, namer: &mut NameMap, fun: &Function) -> String {
    match *inst.operator() {
        op::BinOp(op, ref lhs, ref rhs, round) => {
            let operator = format!("{}{}", binary_op(op), rounding(round));
            assemble(&operator, None, inst.t(), &[Inst(inst), Op(lhs), Op(rhs)], namer)
        },
        op::Mul(ref lhs, ref rhs, rounding, return_type) => {
            let operator = if rounding == op::Rounding::Exact {
                format!("mul{}", mul_mode(lhs.t(), return_type, fun))
            } else {
                format!("mul{}", self::rounding(rounding))
            };
            let t = lower_type(lhs.t(), fun);
            assemble(&operator, None, t, &[Inst(inst), Op(lhs), Op(rhs)], namer)
        },
        op::Mad(ref mul_lhs, ref mul_rhs, ref add_rhs, rounding) => {
            let operator = if rounding == op::Rounding::Exact {
                format!("mad{}", mul_mode(mul_lhs.t(), add_rhs.t(), fun))
            } else {
                format!("fma{}", self::rounding(rounding))
            };
            let args = &[Inst(inst), Op(mul_lhs), Op(mul_rhs), Op(add_rhs)];
            assemble(&operator, None, lower_type(mul_lhs.t(), fun), args, namer)
        },
        op::Mov(ref op) =>
            assemble("mov", None, inst.t(), &[Inst(inst), Op(op)], namer),
        op::Ld(_, ref addr, _) => {
            let operator = ld_operator(unwrap!(inst.mem_flag()));
            assemble(operator, None, inst.t(), &[Inst(inst), Addr(addr)], namer)
        },
        op::St(ref addr, ref val, _,  _) => {
            let operator = st_operator(unwrap!(inst.mem_flag()));
            let t = lower_type(val.t(), fun);
            let guard = if inst.has_side_effects() {
                namer.side_effect_guard()
            } else { None };
            assemble(operator, guard, t, &[Addr(addr), Op(val)], namer)
        },
        op::Cast(ref op, t) => {
            let rounding = match (op.t(), t) {
                (Type::F(_), Type::I(_)) => ".rni",
                (Type::I(_), Type::F(_)) => ".rn",
                (Type::F(x), Type::F(y)) => if x > y { ".rn" } else { "" },
                _ => "",
            };
            let operator = format!("cvt{}.{}", rounding, ptx_type(lower_type(t, fun)));
            let t = lower_type(op.t(), fun);
            assemble(&operator, None, t, &[Inst(inst), Op(op)], namer)
        },
        op::TmpLd(..) | op::TmpSt(..) => panic!("non-printable instruction")
    }
}

/// Prints a vector instruction.
fn vector_inst(inst: &Instruction, dim: &Dimension, namer: &mut NameMap, fun: &Function)
    -> String
{
    let size = unwrap!(dim.size().as_int());
    let flag = unwrap!(inst.mem_flag());
    match *inst.operator() {
        op::Ld(_, ref addr, _) => {
            let operator = format!("{}.v{}", ld_operator(flag), size);
            let args = [VecInst(inst, dim.id(), size), Addr(addr)];
            assemble(&operator, None, inst.t(), &args, namer)
        },
        op::St(ref addr, ref val, _, _) => {
            let operator = format!("{}.v{}", st_operator(flag), size);
            let operands = [Addr(addr), VecOp(val, dim.id(), size)];
            let guard = if inst.has_side_effects() {
                namer.side_effect_guard()
            } else { None };
            assemble(&operator, guard, lower_type(val.t(), fun), &operands, namer)
        },
        _ => panic!("non-vectorizable instruction"),
    }
}

/// Prints the variables declared by the `Namer`.
fn var_decls(namer: &Namer) -> String {
    let print_decl = |(&t, n)| {
        let prefix = Namer::gen_prefix(&t);
        format!(".reg.{} %{}<{}>;", ptx_type(t), prefix, n)
    };
    namer.num_var.iter().map(print_decl).collect_vec().join("\n  ")
}

/// Prints a cfg.
fn cfg<'a>(fun: &Function, c: &Cfg<'a>, namer: &mut NameMap) -> String {
    match *c {
        Cfg::Root(ref cfgs) => cfg_vec(fun, cfgs, namer),
        Cfg::Loop(ref dim, ref cfgs) => ptx_loop(fun, dim, cfgs, namer),
        Cfg::Threads(ref dims, ref ind_levels, ref inner) => {
                let mut res = enable_threads(fun, dims, namer);
                for level in ind_levels {
                    res.push_str(&parallel_induction_level(level, namer));
                    res.push_str("\n  ");
                }
                res.push_str(&cfg_vec(fun, inner, namer));
                res.push_str("\n  bar.sync 0;");
                res
        }
        Cfg::Instruction(ref i) => inst(i, namer, fun),
    }
}

/// Change the side-effect guards so that only thre specified threads are enabled.
fn enable_threads(fun: &Function, threads: &[bool], namer: &mut NameMap) -> String {
    let mut ops = String::new();
    let mut guard = None;
    for (&is_active, dim) in threads.iter().zip_eq(fun.thread_dims().iter()) {
        if is_active { continue; }
        let new_guard = namer.gen_name(ir::Type::I(1));
        let index = namer.name_index(dim.id());
        unwrap!(writeln!(ops, "  setp.eq.s32 {}, {}, 0;", new_guard, index));
        if let Some(ref guard) = guard {
            unwrap!(writeln!(ops, "  and.pred {}, {}, {};", guard, guard, new_guard));
        } else {
            guard = Some(new_guard);
        };
    }
    namer.set_side_effect_guard(guard.map(RcStr::new));
    ops
}

/// Prints a multiplicative induction var level.
fn parallel_induction_level(level: &InductionLevel, namer: &NameMap) -> String {
    let dim_id = level.increment.map(|(dim, _)| dim);
    let ind_var = namer.name_induction_var(level.ind_var, dim_id);
    let base_components =  level.base.components().map(|v| namer.name(v)).collect_vec();
    if let Some((dim, increment)) = level.increment {
        let index = namer.name_index(dim);
        let step = namer.name_size(increment, Type::I(32));
        let mode = if level.t() == Type::I(64) { "wide" } else { "lo" };
        match base_components[..] {
            [] => format!("mul.{}.s32 {}, {}, {};", mode, ind_var, index, step),
            [ref base] =>
                format!("mad.{}.s32 {}, {}, {}, {};", mode, ind_var, index, step, base),
            _ => panic!()
        }
    } else {
        let t = ptx_type(level.t());
        match base_components[..] {
            [] => format!("mov.{} {}, 0;", t, ind_var),
            [ref base] => format!("mov.{} {}, {};", t, ind_var, base),
            [ref lhs, ref rhs] => format!("add.{} {}, {}, {};", t, ind_var, lhs, rhs),
            _ => panic!()
        }
    }
}

/// Prints a vector of cfgs.
fn cfg_vec(fun: &Function, cfgs: &[Cfg], namer: &mut NameMap) -> String {
    cfgs.iter().map(|c| cfg(fun, c, namer)).collect_vec().join("\n  ")
}

/// Prints a loop.
fn ptx_loop(fun: &Function, dim: &Dimension, cfgs: &[Cfg], namer: &mut NameMap)
    -> String
{
    match dim.kind() {
        DimKind::LOOP => {
            let idx = namer.name_index(dim.id()).to_string();
            let ind_levels = dim.induction_levels().iter();
            let (var_init, var_step): (String, String) = ind_levels.map(|level| {
                let t = ptx_type(level.t());
                let dim_id = level.increment.map(|(dim, _)| dim);
                let ind_var = namer.name_induction_var(level.ind_var, dim_id);
                let base_components = level.base.components().map(|v| namer.name(v));
                let init = match base_components.collect_vec()[..] {
                    [ref base] => format!("mov.{} {}, {};\n  ", t, ind_var, base),
                    [ref lhs, ref rhs] =>
                        format!("add.{} {}, {}, {};\n  ", t, ind_var, lhs, rhs),
                    _ => panic!(),
                };
                let step = if let Some((_, increment)) = level.increment {
                    let step = namer.name_size(increment, level.t());
                    format!("add.{} {}, {}, {};\n  ", t, ind_var, step, ind_var)
                } else { String::new() };
                (init, step)
            }).unzip();
            let pred = namer.allocate_pred();
            let loop_id = namer.gen_loop_id();
            format!(include_str!("template/loop.ptx"),
                id = loop_id,
                body = cfg_vec(fun, cfgs, namer),
                idx = idx,
                size = namer.name_size(dim.size(), Type::I(32)),
                pred = pred,
                induction_var_init = var_init,
                induction_var_step = var_step,
            )
        },
        DimKind::UNROLL => {
            let mut body = Vec::new();
            let mut incr_levels = Vec::new();
            for level in dim.induction_levels() {
                let t = ptx_type(level.t());
                let dim_id = level.increment.map(|(dim, _)| dim);
                let ind_var = namer.name_induction_var(level.ind_var, dim_id).to_string();
                let base_components = level.base.components().map(|v| namer.name(v));
                let base = match base_components.collect_vec()[..] {
                    [ref base] => base.to_string(),
                    [ref lhs, ref rhs] => {
                        let tmp = namer.gen_name(level.t());
                        body.push(format!("add.{} {}, {}, {};", t, tmp, lhs, rhs));
                        tmp
                    },
                    _ => panic!(),
                };
                body.push(format!("mov.{} {}, {};", t, ind_var, base));
                if let Some((_, incr)) = level.increment {
                    incr_levels.push((level, ind_var, t, incr, base));
                }
            }
            for i in 0..unwrap!(dim.size().as_int()) {
                namer.set_current_index(dim, i);
                if i > 0 {
                    for &(level, ref ind_var, ref t, incr, ref base) in &incr_levels {
                        let incr =  if let Some(step) = incr.as_int() {
                            format!("add.{} {}, {}, {};", t, ind_var, step*i, base)
                        } else {
                            let step = namer.name_size(incr, level.t());
                            format!("add.{} {}, {}, {};", t, ind_var, step, ind_var)
                        };
                        body.push(incr);
                    }
                }
                body.push(cfg_vec(fun, cfgs, namer));
            }
            namer.unset_current_index(dim);
            body.join("\n  ")
        },
        DimKind::VECTOR => match *cfgs {
            [Cfg::Instruction(ref inst)] => vector_inst(inst, dim, namer, fun),
            ref body => panic!("invalid vector dimension body: {:?}", body),
        },
        kind => panic!("invalid loop kind for ptx printing: {:?}", kind)
    }
}

/// Declares a shared memory block.
fn shared_mem_decl(block: &InternalMemBlock, namer: &mut NameMap) -> String {
    let ptr_type_name = ptx_type(Type::I(32));
    let name = namer.name_addr(block.id());
   format!(".shared.align 16 .u8 {vec_name}[{size}];\
            \n  mov.{t} {name}, {vec_name};",
           vec_name = &name[1..],
           name = name,
           t = ptr_type_name,
           size = unwrap!(block.alloc_size().as_int()))
}

/// Prints PTX to compute the address of the private part of a global memory block.
fn privatise_global_block(block: &InternalMemBlock, namer: &mut NameMap, fun: &Function,
                          gpu: &Gpu) -> String {
    if fun.block_dims().is_empty() { return "".to_string(); }
    let addr = namer.name_addr(block.id());
    let size = namer.name_size(block.local_size(), Type::I(32));
    let d0 = namer.name_index(fun.block_dims()[0].id()).to_string();
    let (var, mut insts) = fun.block_dims()[1..].iter()
        .fold((d0, vec![]), |(old_var, mut insts), dim| {
            let var = namer.gen_name(Type::I(32));
            let size = namer.name_size(dim.size(), Type::I(32));
            let idx = namer.name_index(dim.id());
            insts.push(format!("mad.lo.s32 {}, {}, {}, {};",
                               var, old_var, size, idx));
            (var, insts)
        });
    let mode = mul_mode(Type::I(32), Type::I(gpu.addr_size), fun);
    insts.push(format!("mad{}.s32 {}, {}, {}, {};",
                       mode, addr, var, size, addr));
    insts.join("\n  ")
}

/// Declares block and thread indexes.
fn decl_par_indexes(function: &Function, namer: &mut NameMap) -> String {
    let mut decls = vec![];
    // Load block indexes.
    for (dim, dir) in function.block_dims().iter().zip(&["x", "y", "z"])  {
       let index = namer.name_index(dim.id());
       decls.push(format!("mov.u32 {}, %ctaid.{};", index, dir));
    }
    // Compute thread indexes.
    for (dim, dir) in function.thread_dims().iter().rev().zip(&["x", "y", "z"]) {
        decls.push(format!("mov.s32 {}, %tid.{};", namer.name_index(dim.id()), dir));
    }
    decls.join("\n  ")
}

/// Prints a `Function`.
pub fn function(function: &Function, gpu: &Gpu) -> String {
    let mut namer = Namer::default();
    let (param_decls, ld_params, idx_loads, mem_decls, body);
    let mut init = Vec::new();
    {
        let name_map = &mut NameMap::new(function, &mut namer);
        param_decls = function.device_code_args()
            .map(|v| param_decl(v, name_map))
            .collect_vec().join(",\n  ");
        ld_params = function.device_code_args().map(|val| {
            format!("ld.param.{t} {var_name}, [{name}];",
                    t = ptx_type(val.t()),
                    var_name = name_map.name_param_val(val.key()),
                    name = name_map.name_param(val.key()))
        }).collect_vec().join("\n  ");
        idx_loads = decl_par_indexes(function, name_map);
        mem_decls = function.mem_blocks().flat_map(|block| {
            match block.alloc_scheme() {
                AllocationScheme::Shared =>
                    Some(shared_mem_decl(block, name_map)),
                AllocationScheme::PrivatisedGlobal =>
                    Some(privatise_global_block(block, name_map, function, gpu)),
                AllocationScheme::Global => None,
            }
        }).format("\n  ").to_string();
        // Compute size casts
        for dim in function.dimensions() {
            if !dim.kind().intersects(DimKind::UNROLL | DimKind::LOOP) { continue; }
            for level in dim.induction_levels() {
                if let Some((_, incr)) = level.increment {
                    let name = name_map.declare_size_cast(incr, level.t());
                    if let Some(name) = name {
                        let ptx_t = ptx_type(level.t());
                        let old_name = name_map.name_size(incr, Type::I(32));
                        init.push(format!("cvt.{}.s32 {}, {};", ptx_t, name, old_name));
                    }
                }
            }
        }
        let ind_levels = function.init_induction_levels().into_iter()
            .chain(function.block_dims().iter().flat_map(|d| d.induction_levels()));
        init.extend(ind_levels.map(|level| parallel_induction_level(level, name_map)));
        body = cfg(function, function.cfg(), name_map);
    }
    let var_decls = var_decls(&namer);
    format!(include_str!("template/device.ptx"),
            sm_major = gpu.sm_major,
            sm_minor = gpu.sm_minor,
            addr_size = gpu.addr_size,
            name = function.name,
            params = param_decls,
            num_thread = function.num_threads(),
            idx_loads = idx_loads,
            ld_params = ld_params,
            mem_decls = mem_decls,
            var_decls = var_decls,
            init = init.join("\n  "),
            body = body
           )
}

/// Retruns the string representation of thread and block sizes on the host.
fn host_3sizes<'a, IT>(dims: IT) -> [String; 3]
        where IT: Iterator<Item=&'a Dimension<'a>>  + 'a {
    let mut sizes = ["1".to_string(), "1".to_string(), "1".to_string()];
    for (i, d) in dims.into_iter().enumerate() {
        assert!(i < 3);
        sizes[i] = host_size(d.size())
    }
    sizes
}

/// Prints a size on the host.
fn host_size(size: &Size) -> String {
    let dividend = size.dividend().iter().map(|p| format!("* {}", &p.name));
    format!("{}{}/{}", size.factor(), dividend.format(""), size.divisor())
}

pub fn host_function(fun: &Function, gpu: &Gpu, out: &mut Write) {
    let block_sizes = host_3sizes(fun.block_dims().iter());
    let thread_sizes = host_3sizes(fun.thread_dims().iter().rev());
    let extern_param_names =  fun.params.iter()
        .map(|x| &x.name as &str).collect_vec().join(", ");
    let mut next_extra_var_id = 0;
    let mut extra_def = vec![];
    let mut extra_cleanup = vec![];
    let params = fun.device_code_args().map(|p| match *p {
        ParamVal::External(p, _) => format!("&{}", p.name),
        ParamVal::Size(size) => {
            let extra_var = format!("_extra_{}", next_extra_var_id);
            next_extra_var_id += 1;
            extra_def.push(format!("int32_t {} = {};", extra_var, host_size(size)));
            format!("&{}", extra_var)
        },
        ParamVal::GlobalMem(_, ref size, _) => {
            let extra_var = format!("_extra_{}", next_extra_var_id);
            next_extra_var_id += 1;
            let size = host_size(size);
            extra_def.push(format!("CUDeviceptr {};", extra_var));
            extra_def.push(format!("CHECK_CUDA(cuMemAlloc(&{}, {}));", extra_var, size));
            extra_cleanup.push(format!("CHECK_CUDA(cuMemFree({}));", extra_var));
            format!("&{}", extra_var)
        },
    }).collect_vec().join(", ");
    let extern_params = fun.params.iter()
        .map(|p| format!("{} {}", host_type(&p.t), p.name))
        .collect_vec().join(", ");
    let res = write!(out, include_str!("template/host.c"),
        name = fun.name,
        ptx_code = function(fun, gpu).replace("\n", "\\n\\\n"),
        extern_params = extern_params,
        extern_param_names = extern_param_names,
        param_vec = format!("{{ {} }}", params),
        extra_def = extra_def.join("  \n"),
        extra_cleanup = extra_cleanup.join("  \n"),
        t_dim_x = &thread_sizes[0],
        t_dim_y = &thread_sizes[1],
        t_dim_z = &thread_sizes[2],
        b_dim_x = &block_sizes[0],
        b_dim_y = &block_sizes[1],
        b_dim_z = &block_sizes[2],
    );
    unwrap!(res);
}
