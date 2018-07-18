extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_cbor;
extern crate telamon;

use serde::ser::{SerializeSeq, Serializer};
use telamon::explorer::choice::ActionEx;
use telamon::ir::dim;
use telamon::ir::mem::InternalId;
use telamon::ir::*;
use telamon::search_space::*;

#[derive(Serialize, Deserialize)]
struct EvaluationEvent {
    bound: f64,
    actions: Vec<ActionEx>,
}

fn main() {
    let mut actions = vec![
        ActionEx::Action(Action::InstFlag(InstId(4), InstFlag::MEM_CS)),
        ActionEx::Action(Action::InstFlag(InstId(1), InstFlag::MEM_CG)),
        ActionEx::Action(Action::InstFlag(InstId(0), InstFlag::MEM_CG)),
        ActionEx::Action(Action::Order(
            BBId::Dim(dim::Id(34)),
            BBId::Inst(InstId(8)),
            Order::BEFORE,
        )),
        ActionEx::Action(Action::Order(
            BBId::Dim(dim::Id(23)),
            BBId::Inst(InstId(8)),
            Order::OUTER,
        )),
        ActionEx::Action(Action::Order(
            BBId::Dim(dim::Id(23)),
            BBId::Inst(InstId(6)),
            Order::OUTER,
        )),
        ActionEx::Action(Action::Order(
            BBId::Dim(dim::Id(23)),
            BBId::Dim(dim::Id(18)),
            Order::OUTER,
        )),
        ActionEx::Action(Action::Order(
            BBId::Dim(dim::Id(21)),
            BBId::Dim(dim::Id(18)),
            Order::INNER,
        )),
        ActionEx::Action(Action::Order(
            BBId::Dim(dim::Id(18)),
            BBId::Inst(InstId(8)),
            Order::AFTER,
        )),
        ActionEx::Action(Action::Order(
            BBId::Dim(dim::Id(15)),
            BBId::Dim(dim::Id(12)),
            Order::OUTER,
        )),
        ActionEx::Action(Action::Order(
            BBId::Dim(dim::Id(0)),
            BBId::Inst(InstId(7)),
            Order::BEFORE,
        )),
        ActionEx::Action(Action::Order(
            BBId::Dim(dim::Id(0)),
            BBId::Inst(InstId(1)),
            Order::AFTER,
        )),
        ActionEx::Action(Action::ThreadMapping(
            dim::Id(11),
            dim::Id(1),
            ThreadMapping::MAPPED,
        )),
        ActionEx::Action(Action::ThreadMapping(
            dim::Id(5),
            dim::Id(1),
            ThreadMapping::MAPPED,
        )),
        ActionEx::Action(Action::ThreadMapping(
            dim::Id(3),
            dim::Id(1),
            ThreadMapping::MAPPED_IN,
        )),
        ActionEx::Action(Action::DimKind(dim::Id(41), DimKind::VECTOR)),
        ActionEx::Action(Action::DimKind(dim::Id(39), DimKind::UNROLL)),
        ActionEx::Action(Action::DimKind(dim::Id(38), DimKind::UNROLL)),
        ActionEx::Action(Action::DimKind(dim::Id(35), DimKind::UNROLL)),
        ActionEx::Action(Action::DimKind(dim::Id(34), DimKind::VECTOR)),
        ActionEx::Action(Action::DimKind(dim::Id(29), DimKind::VECTOR)),
        ActionEx::Action(Action::DimKind(dim::Id(23), DimKind::UNROLL)),
        ActionEx::LowerLayout {
            mem: InternalId(1),
            st_dims: vec![dim::Id(36), dim::Id(37), dim::Id(38)],
            ld_dims: vec![dim::Id(39), dim::Id(40), dim::Id(41)],
        },
        ActionEx::Action(Action::DimKind(dim::Id(14), DimKind::THREAD)),
        ActionEx::LowerLayout {
            mem: InternalId(0),
            st_dims: vec![dim::Id(30), dim::Id(32), dim::Id(31)],
            ld_dims: vec![dim::Id(33), dim::Id(35), dim::Id(34)],
        },
        ActionEx::Action(Action::DimKind(dim::Id(12), DimKind::UNROLL)),
        ActionEx::Action(Action::DimKind(dim::Id(11), DimKind::THREAD)),
        ActionEx::Action(Action::DimKind(dim::Id(9), DimKind::BLOCK)),
        ActionEx::Action(Action::DimKind(dim::Id(8), DimKind::THREAD)),
        ActionEx::Action(Action::DimKind(dim::Id(7), DimKind::VECTOR)),
        ActionEx::Action(Action::DimKind(dim::Id(5), DimKind::THREAD)),
        ActionEx::Action(Action::DimKind(dim::Id(3), DimKind::THREAD)),
        ActionEx::Action(Action::DimKind(dim::Id(2), DimKind::LOOP)),
        ActionEx::Action(Action::DimKind(dim::Id(0), DimKind::LOOP)),
        ActionEx::Action(Action::DimKind(dim::Id(1), DimKind::THREAD)),
        ActionEx::TileSizes(vec![vec![32, 4], vec![32, 4], vec![32]]),
    ];
    actions.reverse();

    {
        let f = std::fs::File::create("actions.cbor").unwrap();
        let mut ser = serde_cbor::Serializer::packed(f);
        let mut seq = ser.serialize_seq(None).unwrap();
        seq.serialize_element(&EvaluationEvent {
            bound: 0.0,
            actions: actions,
        }).unwrap();
        seq.end().unwrap();
    }
}
