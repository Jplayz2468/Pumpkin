//! Stateful body/head rotation after movement, compared with Java BodyRotationControl.
use pumpkin_util::math::rotate_if_necessary;

#[derive(Default)]
pub struct BodyRotation {
    pub stable_ticks: i32,
    pub stable_head: f32,
}

impl BodyRotation {
    pub fn tick(
        &mut self,
        displacement: [f64; 2],
        yaw: f32,
        mut body: f32,
        mut head: f32,
        max_head: f32,
        first_passenger_is_mob: bool,
    ) -> [f32; 2] {
        if displacement[0] * displacement[0] + displacement[1] * displacement[1]
            > f64::from(2.500_000_3e-7_f32)
        {
            body = yaw;
            head = rotate_if_necessary(head, body, max_head);
            self.stable_head = head;
            self.stable_ticks = 0;
        } else if !first_passenger_is_mob {
            if (head - self.stable_head).abs() > 15.0 {
                self.stable_ticks = 0;
                self.stable_head = head;
                body = rotate_if_necessary(body, head, max_head);
            } else {
                self.stable_ticks = self.stable_ticks.wrapping_add(1);
                if self.stable_ticks > 10 {
                    let fraction =
                        ((self.stable_ticks.wrapping_sub(10)) as f32 / 10.0).clamp(0.0, 1.0);
                    body = rotate_if_necessary(body, head, max_head * (1.0 - fraction));
                }
            }
        }
        [body, head]
    }
}
