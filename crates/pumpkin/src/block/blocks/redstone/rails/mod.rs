use pumpkin_data::Block;
use pumpkin_data::BlockStateId;
use pumpkin_data::block_properties::HorizontalFacing;
use pumpkin_data::block_properties::PoweredRailLikeProperties;
use pumpkin_data::block_properties::RailLikeProperties;
use pumpkin_data::block_properties::RailShape;
use pumpkin_data::block_properties::RailShapeStraight;

mod common;

pub mod activator_rail;
pub mod detector_rail;
pub mod powered_rail;
pub mod rail;

enum RailProperties {
    Rail(RailLikeProperties),
    StraightRail(PoweredRailLikeProperties),
}

impl RailProperties {
    pub fn default(block: &Block) -> Self {
        if block == &Block::RAIL {
            Self::Rail(RailLikeProperties::default(block))
        } else {
            Self::StraightRail(PoweredRailLikeProperties::default(block))
        }
    }

    pub fn new(state_id: BlockStateId, block: &Block) -> Self {
        if block == &Block::RAIL {
            Self::Rail(RailLikeProperties::from_state_id(state_id))
        } else {
            Self::StraightRail(PoweredRailLikeProperties::from_state_id(state_id))
        }
    }

    const fn shape(&self) -> RailShape {
        match self {
            Self::Rail(props) => props.shape,
            Self::StraightRail(props) => match props.shape {
                RailShapeStraight::NorthSouth => RailShape::NorthSouth,
                RailShapeStraight::EastWest => RailShape::EastWest,
                RailShapeStraight::AscendingEast => RailShape::AscendingEast,
                RailShapeStraight::AscendingWest => RailShape::AscendingWest,
                RailShapeStraight::AscendingNorth => RailShape::AscendingNorth,
                RailShapeStraight::AscendingSouth => RailShape::AscendingSouth,
            },
        }
    }

    fn to_state_id(&self, block: &Block) -> BlockStateId {
        match self {
            Self::Rail(props) => props.to_state_id(block),
            Self::StraightRail(props) => props.to_state_id(block),
        }
    }

    const fn set_waterlogged(&mut self, waterlogged: bool) {
        match self {
            Self::Rail(props) => props.waterlogged = waterlogged,
            Self::StraightRail(props) => props.waterlogged = waterlogged,
        }
    }

    fn set_shape(&mut self, shape: RailShape) {
        match self {
            Self::Rail(props) => props.shape = shape,
            Self::StraightRail(props) => {
                props.shape = match shape {
                    RailShape::NorthSouth => RailShapeStraight::NorthSouth,
                    RailShape::EastWest => RailShapeStraight::EastWest,
                    RailShape::AscendingEast => RailShapeStraight::AscendingEast,
                    RailShape::AscendingWest => RailShapeStraight::AscendingWest,
                    RailShape::AscendingNorth => RailShapeStraight::AscendingNorth,
                    RailShape::AscendingSouth => RailShapeStraight::AscendingSouth,
                    _ => {
                        tracing::error!("Trying to make a straight rail curved: {:?}", shape);
                        return;
                    }
                }
            }
        }
    }

    const fn is_powered(&self) -> bool {
        match self {
            Self::Rail(_) => false,
            Self::StraightRail(props) => props.powered,
        }
    }

    const fn set_powered(&mut self, powered: bool) {
        match self {
            Self::Rail(_) => {}
            Self::StraightRail(props) => props.powered = powered,
        }
    }
}

pub trait StraightRailShapeExt {
    fn as_shape(&self) -> RailShape;
}

impl StraightRailShapeExt for RailShapeStraight {
    fn as_shape(&self) -> RailShape {
        match self {
            Self::NorthSouth => RailShape::NorthSouth,
            Self::EastWest => RailShape::EastWest,
            Self::AscendingNorth => RailShape::AscendingNorth,
            Self::AscendingSouth => RailShape::AscendingSouth,
            Self::AscendingEast => RailShape::AscendingEast,
            Self::AscendingWest => RailShape::AscendingWest,
        }
    }
}

pub trait HorizontalFacingRailExt {
    fn to_rail_shape_flat(&self) -> RailShapeStraight;
    fn to_rail_shape_ascending_towards(&self) -> RailShapeStraight;
}

impl HorizontalFacingRailExt for HorizontalFacing {
    fn to_rail_shape_flat(&self) -> RailShapeStraight {
        match self {
            Self::North | Self::South => RailShapeStraight::NorthSouth,
            Self::East | Self::West => RailShapeStraight::EastWest,
        }
    }

    fn to_rail_shape_ascending_towards(&self) -> RailShapeStraight {
        match self {
            Self::North => RailShapeStraight::AscendingNorth,
            Self::South => RailShapeStraight::AscendingSouth,
            Self::East => RailShapeStraight::AscendingEast,
            Self::West => RailShapeStraight::AscendingWest,
        }
    }
}
