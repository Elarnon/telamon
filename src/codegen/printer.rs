use codegen::*;
use utils::*;
use itertools::Itertools;
use search_space::InstFlag;

use ir::{self, op, Type};
use search_space::DimKind;

#[derive(Copy, Clone)]
pub enum MulMode {
    Wide,
    Low,
    High,
    Empty,
}
pub trait Printer {

    /// Get a proper string representation of an integer in target language
    fn get_int(n: u32) -> String;

    /// Get a proper string representation of an float in target language
    fn get_float(f: f64) -> String;

    /// Print a type in the backend
    fn get_type(t: Type) -> String;

    /// Print return_id = lhs op rhs
    fn print_binop(&mut self, op: ir::BinOp, return_type: Type, rounding: op::Rounding, return_id: &str, lhs: &str, rhs: &str); 

    /// Print return_id = op1 * op2
    fn print_mul(&mut self, return_type: Type, round: op::Rounding, mul_mode: MulMode, return_id: &str, op1: &str, op2: &str);

    /// Print return_id = mlhs * mrhs + arhs
    fn print_mad(&mut self, ret_type: Type, round: op::Rounding, mul_mode: MulMode, return_id: &str,  mlhs: &str, mrhs: &str, arhs: &str);

    /// Print return_id = op 
    fn print_mov(&mut self, return_type: Type, return_id: &str, op: &str);

    /// Print return_id = load [addr] 
    fn print_ld(&mut self, return_type: Type,flag: InstFlag, return_id: &str,  addr: &str);

    /// Print store val [addr] 
    fn print_st(&mut self, val_type: Type, mem_flag: InstFlag, addr: &str, val: &str);

    /// Print if (cond) store val [addr] 
    fn print_cond_st(&mut self, return_type: Type, mem_flag: InstFlag, cond: &str, addr: &str, val: &str);

    /// Print return_id = (val_type) val
    fn print_cast(&mut self, t: Type, round: op::Rounding, return_id: &str, op1: &str);

    /// print a label where to jump
    fn print_label(&mut self, label_id: &str);

    /// Print return_id = op1 && op2
    fn print_and(&mut self, return_id: &str, op1: &str, op2: &str);

    /// Print return_id = op1 || op2
    fn print_or(&mut self, return_id: &str, op1: &str, op2: &str);

    /// Print return_id = op1 == op2
    fn print_equal(&mut self, return_id: &str, op1: &str, op2: &str);

    /// Print return_id = op1 < op2
    fn print_lt(&mut self, return_id: &str, op1: &str, op2: &str);

    /// Print return_id = op1 > op2
    fn print_gt(&mut self, return_id: &str, op1: &str, op2: &str);

    /// Print if (cond) jump label(label_id)
    fn print_cond_jump(&mut self, label_id: &str, cond: &str);

    /// Print wait on all threads
    fn print_sync(&mut self);

    fn mul_mode(from: Type, to: Type) -> MulMode {
        match (from, to) {
            (Type::I(32), Type::I(64)) => MulMode::Wide,
            (ref x, ref y) if x == y => MulMode::Low,
            _ => panic!(),
        }
    }

    // TODO(cleanup): remove this function once values are preprocessed by codegen. If values
    // are preprocessed, types will be already lowered.
    fn lower_type(t: ir::Type, fun: &Function) -> ir::Type {
        unwrap!(fun.space().ir_instance().device().lower_type(t, fun.space()))
    }

    fn cfg_vec(&mut self, fun: &Function, cfgs: &[Cfg], namer: &mut NameMap) {
        for c in cfgs.iter() {
            self.cfg( fun, c, namer);
        }
    }

    /// Prints a cfg.
    fn cfg<'a>(&mut self, fun: &Function, c: &Cfg<'a>, namer: &mut NameMap) {
        match *c {
            Cfg::Root(ref cfgs) => self.cfg_vec( fun, cfgs, namer),
            Cfg::Loop(ref dim, ref cfgs) => self.gen_loop( fun, dim, cfgs, namer),
            Cfg::Threads(ref dims, ref ind_levels, ref inner) => {
                self.enable_threads( fun, dims, namer);
                for level in ind_levels {
                    self.parallel_induction_level( level, namer);
                }
                self.cfg_vec( fun, inner, namer);
                self.print_sync();
            }
            Cfg::Instruction(ref i) => self.inst( i, namer, fun),
        }
    }

    /// Prints a multiplicative induction var level.
    fn parallel_induction_level(&mut self, level: &InductionLevel, namer: &NameMap) {
        let dim_id = level.increment.map(|(dim, _)| dim);
        let ind_var = namer.name_induction_var(level.ind_var, dim_id);
        let base_components =  level.base.components().map(|v| namer.name(v)).collect_vec();
        if let Some((dim, increment)) = level.increment {
            let index = namer.name_index(dim);
            let step = namer.name_size(increment, Type::I(32));
            let mode = if level.t() == Type::I(64) { MulMode::Wide } else { MulMode::Low };
            match base_components[..] {
                // TODO get a sensible type for index and step rather than just random I32
                [] => self.print_mul(level.t(), op::Rounding::Exact, mode, &ind_var, &index, &step),
                [ref base] => self.print_mad(level.t(), op::Rounding::Exact, mode, &ind_var, &index, &step, &base),
                _ => panic!()
            }
        } else {
            match base_components[..] {
                [] => {let zero = Self::get_int(0); self.print_mov(level.t(), &ind_var, &zero);}
                [ref base] =>  self.print_mov(level.t(), &ind_var, &base),
                [ref lhs, ref rhs] =>  self.print_binop(ir::BinOp::Add, level.t(), op::Rounding::Exact, &ind_var, &lhs, &rhs),
                _ => panic!()
            }
        }
    }

    /// Change the side-effect guards so that only the specified threads are enabled.
    fn enable_threads(&mut self, fun: &Function, threads: &[bool], namer: &mut NameMap) {
        let mut guard = None;
        for (&is_active, dim) in threads.iter().zip_eq(fun.thread_dims().iter()) {
            if is_active { continue; }
            let new_guard = namer.gen_name(ir::Type::I(1));
            let index = namer.name_index(dim.id());
            self.print_equal(&new_guard, index, &"0");
            if let Some(ref guard) = guard {
                self.print_and(&guard as &str, &guard as &str, &new_guard);
            } else {
                guard = Some(new_guard);
            };
        }
        namer.set_side_effect_guard(guard.map(RcStr::new));
    }


    fn gen_loop(&mut self, fun: &Function, dim: &Dimension, cfgs:
                                &[Cfg], namer: &mut NameMap)
    {
        match dim.kind() {
            DimKind::LOOP => {
                self.standard_loop(fun, dim, cfgs, namer)
            }
            DimKind::UNROLL => {self.unroll_loop(fun, dim, cfgs, namer)}
            DimKind::VECTOR => {self.unroll_loop(fun, dim, cfgs, namer)}
            _ => { unimplemented!() }
        }
    }

    fn standard_loop(&mut self, fun: &Function, dim: &Dimension, cfgs:
                                     &[Cfg], namer: &mut NameMap) {
        let idx = namer.name_index(dim.id()).to_string();
        let zero = Self::get_int(0);
        self.print_mov(Type::I(32), &idx, &zero);
        let mut ind_var_vec = vec![];
        let loop_id = namer.gen_loop_id();
        let ind_levels = dim.induction_levels();
        for level in ind_levels.iter() {
            let dim_id = level.increment.map(|(dim, _)| dim);
            let ind_var = namer.name_induction_var(level.ind_var, dim_id);
            let base_components = level.base.components().map(|v| namer.name(v));
            match base_components.collect_vec()[..] {
                [ref base] => self.print_mov(level.t(), &ind_var, &base),
                [ref lhs, ref rhs] =>
                    self.print_binop( ir::BinOp::Add, level.t(), op::Rounding::Exact, &ind_var, &lhs, &rhs, ),
                _ => panic!(),
            };
            ind_var_vec.push(ind_var.into_owned());
        }
        self.print_label(&loop_id.to_string());
        self.cfg_vec(fun, cfgs, namer);
        for (level, ind_var) in ind_levels.iter().zip_eq(ind_var_vec) {
            if let Some((_, increment)) = level.increment {
                let step = namer.name_size(increment, level.t());
                self.print_binop(ir::BinOp::Add, level.t(), op::Rounding::Exact, &ind_var, &ind_var, &step);
            };
        }
        let one = Self::get_int(1);
        self.print_binop(op::BinOp::Add, Type::I(32), op::Rounding::Exact, &idx, &idx,  &one);
        let gt_cond = namer.gen_name(ir::Type::I(1));
        self.print_lt(&gt_cond, &idx, &namer.name_size(dim.size(), Type::I(32)));
        self.print_cond_jump(&loop_id.to_string(), &gt_cond);
    }

    fn unroll_loop(&mut self, fun: &Function, dim: &Dimension, cfgs: &[Cfg], namer: &mut NameMap) {
        //let mut body = Vec::new();
        let mut incr_levels = Vec::new();
        for level in dim.induction_levels() {
            let dim_id = level.increment.map(|(dim, _)| dim);
            let ind_var = namer.name_induction_var(level.ind_var, dim_id).to_string();
            let base_components = level.base.components().map(|v| namer.name(v));
            let base = match base_components.collect_vec()[..] {
                [ref base] => base.to_string(),
                [ref lhs, ref rhs] => {
                    let tmp = namer.gen_name(level.t());
                    self.print_binop(ir::BinOp::Add, level.t(), op::Rounding::Exact, &tmp, lhs, rhs);
                    tmp
                },
                _ => panic!(),
            };
            self.print_mov(level.t(), &ind_var, &base);
            if let Some((_, incr)) = level.increment {
                incr_levels.push((level, ind_var, incr, base));
            }
        }
        for i in 0..unwrap!(dim.size().as_int()) {
            namer.set_current_index(dim, i);
            if i > 0 {
                for &(level, ref ind_var, incr, ref base) in &incr_levels {
                    if let Some(step) = incr.as_int() {
                        let stepxi = Self::get_int(step * i);
                        self.print_binop(ir::BinOp::Add, level.t(), op::Rounding::Exact, ind_var, &stepxi, base);
                    } else {
                        let step = namer.name_size(incr, level.t());
                        self.print_binop(ir::BinOp::Add, level.t(), op::Rounding::Exact, &ind_var, &step, &ind_var);
                    };
                }
            }
            self.cfg_vec(fun, cfgs, namer);
        }
        namer.unset_current_index(dim);
    }

    fn privatise_global_block(&mut self, block: &InternalMemBlock,
                                              namer: &mut NameMap, fun: &Function) {
        if fun.block_dims().is_empty() { return ; }
        let addr = namer.name_addr(block.id());
        let addr_type = Self::lower_type(Type::PtrTo(block.id().into()), fun);
        let size = namer.name_size(block.local_size(), Type::I(32));
        let d0 = namer.name_index(fun.block_dims()[0].id()).to_string();
        let var = fun.block_dims()[1..].iter()
            .fold(d0, |old_var, dim| {
                let var = namer.gen_name(Type::I(32));
                let size = namer.name_size(dim.size(), Type::I(32));
                let idx = namer.name_index(dim.id());
                self.print_mad(Type::I(32), op::Rounding::Exact, MulMode::Low, &var,  &old_var, &size, &idx);
                var
            });
        let mode = Self::mul_mode(Type::I(32), addr_type);
        self.print_mad(addr_type, op::Rounding::Exact, mode, &addr,  &var, &size, &addr);
    }

    /// Prints an instruction.
    fn inst(&mut self, inst: &Instruction, namer: &mut NameMap, fun: &Function ) {
        match *inst.operator() {
            op::BinOp(op, ref lhs, ref rhs, round) => {
                self.print_binop(op, inst.t(), round, &namer.name_inst(inst), &namer.name_op(lhs), &namer.name_op(rhs))
            }
            op::Mul(ref lhs, ref rhs, round, return_type) => {
                let low_lhs_type = Self::lower_type(lhs.t(), fun);
                let low_ret_type = Self::lower_type(return_type, fun);
                let mode = if round == op::Rounding::Exact {
                    Self::mul_mode(low_lhs_type, low_ret_type)
                } else {MulMode::Empty };
                self.print_mul(low_ret_type, round, mode, &namer.name_inst(inst), &namer.name_op(lhs), &namer.name_op(rhs))
            },
            op::Mad(ref mul_lhs, ref mul_rhs, ref add_rhs, round) => {
                let low_mlhs_type = Self::lower_type(mul_lhs.t(), fun);
                let low_arhs_type = Self::lower_type(add_rhs.t(), fun);
                let mode = if round == op::Rounding::Exact {
                    Self::mul_mode(low_mlhs_type, low_arhs_type)
                } else {MulMode::Empty };
                self.print_mad(Self::lower_type(inst.t(), fun), round, mode, &namer.name_inst(inst), &namer.name_op(mul_lhs), &namer.name_op(mul_rhs), &namer.name_op(add_rhs))
            },
            op::Mov(ref op) => {

                self.print_mov(Self::lower_type(inst.t(), fun), &namer.name_inst(inst), &namer.name_op(op))
            },
            op::Ld(ld_type, ref addr, _) => {

                self.print_ld(ld_type, unwrap!(inst.mem_flag()), &namer.name_inst(inst), &namer.name_op(addr))
            },
            op::St(ref addr, ref val, _,  _) => {
                let guard = if inst.has_side_effects() {
                    namer.side_effect_guard()
                } else { None };
                if let Some(ref pred) = guard {
                    self.print_cond_st(Self::lower_type(val.t(), fun), unwrap!(inst.mem_flag()), pred, &namer.name_op(addr), &namer.name_op(val) );
                } else {self.print_st(Self::lower_type(val.t(), fun), unwrap!(inst.mem_flag()), &namer.name_op(addr), &namer.name_op(val));};
            },
            op::Cast(ref op, t) => {
                let rounding = match (op.t(), t) {
                    (Type::F(_), Type::I(_)) => op::Rounding::Nearest,
                    (Type::I(_), Type::F(_)) => op::Rounding::Nearest,
                    (Type::F(x), Type::F(y)) => if x > y {op::Rounding::Nearest} else {  op::Rounding::Exact },
                    _ => op::Rounding::Exact,
                };
                self.print_cast(t, rounding, &namer.name_inst(inst), &namer.name_op(op))
            },
            op::TmpLd(..) | op::TmpSt(..) => panic!("non-printable instruction")
        }
    }
}
