//! Java axis-ordered movement against a list of box collision shapes.
#[derive(Clone, Copy)]
pub struct Box3 {
    pub min: [f64; 3],
    pub max: [f64; 3],
}
pub fn collide(motion: [f64; 3], bounds: Box3, shapes: &[Box3]) -> ([f64; 3], Option<usize>) {
    if shapes.is_empty() {
        return (motion, None);
    }
    let order = if motion[0].abs() < motion[2].abs() {
        [1, 2, 0]
    } else {
        [1, 0, 2]
    };
    let mut result = [0.0; 3];
    let mut support = None;
    for axis in order {
        if motion[axis] == 0.0 {
            continue;
        }
        let min: [f64; 3] = std::array::from_fn(|i| bounds.min[i] + result[i]);
        let max: [f64; 3] = std::array::from_fn(|i| bounds.max[i] + result[i]);
        let mut movement = motion[axis];
        for (index, shape) in shapes.iter().enumerate() {
            if movement.abs() < 1.0e-7 {
                movement = 0.0;
                break;
            }
            if (0..3).any(|i| {
                i != axis && (min[i] + 1.0e-7 >= shape.max[i] || max[i] - 1.0e-7 < shape.min[i])
            }) {
                continue;
            }
            let before = movement;
            if movement > 0.0 && max[axis] - 1.0e-7 < shape.min[axis] {
                let gap = shape.min[axis] - max[axis];
                if gap >= -1.0e-7 {
                    movement = movement.min(gap);
                }
            } else if movement < 0.0 && min[axis] + 1.0e-7 >= shape.max[axis] {
                let gap = shape.max[axis] - min[axis];
                if gap <= 1.0e-7 {
                    movement = movement.max(gap);
                }
            }
            if axis == 1 && motion[1] < 0.0 && before != movement {
                support = Some(index);
            }
        }
        result[axis] = movement;
    }
    (result, support)
}
