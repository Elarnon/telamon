//! Compute and represent local information on the different objects representing of the IR.
use device::{Context, Device};
use ir::{self, Statement};
use itertools::Itertools;
use model::{HwPressure, size};
use num::integer::lcm;
use search_space::{DimKind, Domain, Order, SearchSpace, ThreadMapping};
use utils::*;

/// Local information on the different objects.
#[derive(Debug)]
pub struct LocalInfo {
    /// The loops inside and outside each BB.
    pub nesting: HashMap<ir::BBId, Nesting>,
    /// The pressure incured by a signle instance of each BB.
    pub hw_pressure: HashMap<ir::BBId, HwPressure>,
    /// The size of each dimensions.
    pub dim_sizes: HashMap<ir::DimId, u32>,
    /// The pressure induced by a single iteration of each dimension and the exit latency
    /// of the loop.
    pub dim_overhead: HashMap<ir::DimId, (HwPressure, HwPressure)>,
    /// The overhead to initialize a thread.
    pub thread_overhead: HwPressure,
    /// Available parallelism in the kernel.
    pub parallelism: Parallelism,
}

impl LocalInfo {
    /// Compute the local information for the given search space, in the context.
    pub fn compute(space: &SearchSpace, context: &Context) -> Self {
        let dim_sizes = space
            .ir_instance()
            .dims()
            .map(|d| (d.id(), context.eval_size(&d.size().clone().into())))
            .collect();
        let nesting: HashMap<_, _> = space
            .ir_instance()
            .blocks()
            .map(|bb| (bb.bb_id(), Nesting::compute(space, bb.bb_id(), &dim_sizes)))
            .collect();
        let mut hw_pressure = space
            .ir_instance()
            .blocks()
            .map(|bb| {
                let is_thread = if let ir::BBId::Dim(id) = bb.bb_id() {
                    space.domain().get_dim_kind(id) == DimKind::THREAD
                } else {
                    false
                };
                // Only keep the pressure of innermost thread dimensions. Otherwise it
                // will be taken multiple times into account.
                let pressure = if is_thread && nesting[&bb.bb_id()].has_inner_thread_dims
                {
                    HwPressure::zero(context.device())
                } else {
                    context
                        .device()
                        .hw_pressure(space, &dim_sizes, &nesting, bb, context)
                };
                (bb.bb_id(), pressure)
            })
            .collect();
        let mut dim_overhead = space
            .ir_instance()
            .dims()
            .map(|d| {
                let kind = space.domain().get_dim_kind(d.id());
                if kind == DimKind::THREAD && nesting[&d.bb_id()].has_inner_thread_dims {
                    // Only keep the overhead on innermost thread dimensions. Otherwise it
                    // will be taken multiple times into account.
                    let zero = HwPressure::zero(context.device());
                    (d.id(), (zero.clone(), zero))
                } else {
                    (d.id(), context.device().loop_iter_pressure(kind))
                }
            })
            .collect();
        let parallelism = parallelism(&nesting, space, context);
        // Add the pressure induced by induction variables.
        let mut thread_overhead = HwPressure::zero(context.device());
        for (_, var) in space.ir_instance().induction_vars() {
            add_indvar_pressure(
                context.device(),
                space,
                &dim_sizes,
                var,
                &mut hw_pressure,
                &mut dim_overhead,
                &mut thread_overhead,
            );
        }
        LocalInfo {
            dim_sizes,
            nesting,
            hw_pressure,
            dim_overhead,
            thread_overhead,
            parallelism,
        }
    }
}

fn add_indvar_pressure(
    device: &Device,
    space: &SearchSpace,
    dim_sizes: &HashMap<ir::DimId, u32>,
    indvar: &ir::InductionVar,
    hw_pressure: &mut HashMap<ir::BBId, HwPressure>,
    dim_overhead: &mut HashMap<ir::DimId, (HwPressure, HwPressure)>,
    thread_overhead: &mut HwPressure,
) {
    for &(dim, _) in indvar.dims() {
        let dim_kind = space.domain().get_dim_kind(dim);
        if dim_kind.intersects(DimKind::VECTOR) {
            continue;
        }
        let t = device
            .lower_type(indvar.base().t(), space)
            .unwrap_or(ir::Type::I(32));
        let mut overhead = if dim_kind.intersects(DimKind::UNROLL | DimKind::LOOP) {
            // FIXME: do not add the latency if the induction level can statically computed.
            // This is the case when:
            // - the loop is unrolled
            // - the increment is a constant
            // - both the conditions are also true for an inner dimension.
            device.additive_indvar_pressure(&t)
        } else {
            device.multiplicative_indvar_pressure(&t)
        };
        let size = dim_sizes[&dim];
        if dim_kind.intersects(DimKind::THREAD | DimKind::BLOCK) {
            thread_overhead.add_parallel(&overhead);
        } else if size > 1 {
            overhead.repeat_parallel(f64::from(size - 1));
            unwrap!(hw_pressure.get_mut(&dim.into())).add_parallel(&overhead);
            overhead.repeat_parallel(1.0 / f64::from(size - 1));
            unwrap!(dim_overhead.get_mut(&dim))
                .0
                .add_parallel(&overhead);
        }
    }
}

/// Nesting of an object.
#[derive(Debug)]
pub struct Nesting {
    /// Dimensions nested inside the current BB.
    pub inner_dims: VecSet<ir::DimId>,
    /// Basic blocks nested inside the current BB.
    pub inner_bbs: VecSet<ir::BBId>,
    /// Dimensions nested outsidethe current BB.
    pub outer_dims: VecSet<ir::DimId>,
    /// Dimensions to be processed before the current BB.
    pub before_self: VecSet<ir::DimId>,
    /// Dimensions that should not take the current BB into account when processed.
    pub after_self: VecSet<ir::DimId>,
    /// The dimensions that can be merged to this one and have a bigger ID.
    pub bigger_merged_dims: VecSet<ir::DimId>,
    /// Indicates if the block may have thread dimensions nested inside it.
    /// Only consider thread dimensions that are sure to be mapped to threads.
    has_inner_thread_dims: bool,
    /// Number of threads that are not represented in the active dimensions of the block.
    pub num_unmapped_threads: u32,
    /// Maximal number of threads this block can be in, considering only outer and mapped
    /// out dimensions.
    pub max_threads_per_block: u64,
}

impl Nesting {
    /// Computes the nesting of a `Statement`.
    fn compute(
        space: &SearchSpace,
        bb: ir::BBId,
        dim_sizes: &HashMap<ir::DimId, u32>,
    ) -> Self {
        let mut inner_dims = Vec::new();
        let mut inner_bbs = Vec::new();
        let mut before_self = Vec::new();
        let mut after_self = Vec::new();
        let mut bigger_merged_dims = Vec::new();
        let mut has_inner_thread_dims = false;
        for other_bb in space.ir_instance().blocks() {
            if other_bb.bb_id() == bb {
                continue;
            }
            let order = space.domain().get_order(other_bb.bb_id(), bb);
            if Order::INNER.contains(order) {
                inner_bbs.push(other_bb.bb_id());
            }
            if let Some(dim) = other_bb.as_dim() {
                let other_kind = space.domain().get_dim_kind(dim.id());
                if Order::INNER.contains(order) {
                    inner_dims.push(dim.id());
                }
                if order.intersects(Order::INNER) {
                    if other_kind == DimKind::THREAD {
                        has_inner_thread_dims = true;
                    }
                }
                if (Order::INNER | Order::BEFORE).contains(order) {
                    before_self.push(dim.id());
                }
                if (Order::OUTER | Order::AFTER).contains(order) {
                    after_self.push(dim.id());
                }
                if order.intersects(Order::MERGED) && other_bb.bb_id() > bb {
                    bigger_merged_dims.push(dim.id());
                }
            }
        }
        let outer_dims = Self::get_iteration_dims(space, bb);
        let num_unmapped_threads = space
            .ir_instance()
            .thread_dims()
            .filter(|dim| {
                !outer_dims.iter().any(|&other| {
                    if dim.id() == other {
                        return true;
                    }
                    if space.ir_instance().dim(other).possible_sizes().is_none() {
                        return false;
                    }
                    let mapping = space.domain().get_thread_mapping(dim.id(), other);
                    mapping.intersects(ThreadMapping::MAPPED)
                })
            })
            .map(|d| dim_sizes[&d.id()])
            .product::<u32>();
        let max_threads_per_block = outer_dims
            .iter()
            .cloned()
            .filter(|&d| space.domain().get_dim_kind(d).intersects(DimKind::THREAD))
            .map(|d| dim_sizes[&d] as u64)
            .product::<u64>()
            * num_unmapped_threads as u64;
        Nesting {
            inner_dims: VecSet::new(inner_dims),
            inner_bbs: VecSet::new(inner_bbs),
            outer_dims,
            before_self: VecSet::new(before_self),
            after_self: VecSet::new(after_self),
            bigger_merged_dims: VecSet::new(bigger_merged_dims),
            has_inner_thread_dims,
            num_unmapped_threads,
            max_threads_per_block,
        }
    }

    /// Computess the list of iteration dimensions of a `Statement`.
    fn get_iteration_dims(space: &SearchSpace, bb: ir::BBId) -> VecSet<ir::DimId> {
        let dims = if let ir::BBId::Inst(inst) = bb {
            space
                .ir_instance()
                .inst(inst)
                .iteration_dims()
                .iter()
                .cloned()
                .collect()
        } else {
            let mut outer = Vec::new();
            for dim in space.ir_instance().dims().map(|d| d.id()) {
                if bb == dim.into() {
                    continue;
                }
                let order = space.domain().get_order(dim.into(), bb);
                if Order::OUTER.contains(order) {
                    if outer.iter().cloned().all(|outer: ir::DimId| {
                        let ord = space.domain().get_order(dim.into(), outer.into());
                        !ord.contains(Order::MERGED)
                    }) {
                        outer.push(dim);
                    }
                }
            }
            outer
        };
        VecSet::new(dims)
    }
}

/// Minimum and maximum parallelism in the whole GPU.
#[derive(Debug)]
pub struct Parallelism {
    /// Minimal number of blocks.
    pub min_num_blocks: u64,
    /// Minimal number of threads per blocks.
    pub min_num_threads_per_blocks: u64,
    /// Minimal number of threads.
    pub min_num_threads: u64,
    /// A multiple of the number of blocks.
    pub lcm_num_blocks: u64,
}

impl Parallelism {
    /// Combines two `Parallelism` summaries computed on different instructions and computes the
    /// `Parallelism` of the union of the instructions.
    fn combine(self, rhs: Self) -> Self {
        let min_num_threads_per_blocks = self.min_num_threads_per_blocks
            .min(rhs.min_num_threads_per_blocks);
        Parallelism {
            min_num_blocks: self.min_num_blocks.min(rhs.min_num_blocks),
            min_num_threads_per_blocks,
            min_num_threads: self.min_num_threads.min(rhs.min_num_threads),
            lcm_num_blocks: lcm(self.lcm_num_blocks, rhs.lcm_num_blocks),
        }
    }
}

impl Default for Parallelism {
    fn default() -> Self {
        Parallelism {
            min_num_blocks: 1,
            min_num_threads_per_blocks: 1,
            min_num_threads: 1,
            lcm_num_blocks: 1,
        }
    }
}

/// Computes the minimal and maximal parallelism accross instructions.
fn parallelism(
    nesting: &HashMap<ir::BBId, Nesting>,
    space: &SearchSpace,
    ctx: &Context,
) -> Parallelism {
    let size_thread_dims = space
        .ir_instance()
        .thread_dims()
        .map(|d| d.size())
        .product::<ir::PartialSize>();
    let min_threads_per_blocks = size::bounds(&size_thread_dims, space, ctx).min;
    space
        .ir_instance()
        .insts()
        .map(|inst| {
            let mut min_size_blocks = ir::PartialSize::default();
            let mut max_size_blocks = ir::PartialSize::default();
            for &dim in &nesting[&inst.bb_id()].outer_dims {
                let kind = space.domain().get_dim_kind(dim);
                if kind.intersects(DimKind::BLOCK) {
                    let size = space.ir_instance().dim(dim).size();
                    max_size_blocks *= size;
                    if kind == DimKind::BLOCK {
                        min_size_blocks *= size;
                    }
                }
            }
            let min_num_blocks =  size::bounds(&min_size_blocks, space, ctx).min;
            let lcm_num_blocks = size::factors(&min_size_blocks, space, ctx).lcm;
            let size_threads_and_blocks = min_size_blocks * &size_thread_dims;
            Parallelism {
                min_num_blocks,
                min_num_threads_per_blocks: min_threads_per_blocks,
                min_num_threads: size::bounds(&size_threads_and_blocks, space, ctx).min,
                lcm_num_blocks,
            }
        })
        .fold1(|lhs, rhs| lhs.combine(rhs))
        .unwrap_or(Parallelism::default())
}
