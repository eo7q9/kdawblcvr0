/// Just draw a circle on screen
extern crate nannou; // pull in functions and data-structues of nannou
extern crate rand; // pull in functionality for random numbers

use nannou::draw::Draw;
use nannou::prelude::*; // load (extended) prelude
use rand::Rng; // pull in random number generator

// data model of a circle
pub struct Circle {
    pub position: Point2, // position = point in 2D space
    pub color: Rgba,      // color
    pub radius: f32,      // determine size
}

#[allow(dead_code)]
impl Circle {
    // create a new (unit) circle at 0.0, 0.0
    pub fn new(c: Rgba) -> Circle {
        Circle {
            position: pt2(0.0, 0.0),
            color: c,
            radius: 1.0,
        }
    }

    // Generate a (new) circle with random position, color, and radius
    pub fn random() -> Circle {
        let mut rng = rand::thread_rng(); // create random number generator
        let rad_val = rng.gen_range(1.0, 15.0); // generate random radius between 1 and (excluding) 15
        let x = rng.gen_range(rad_val, 100.0); // generate random x position (radius as lower bound to have full circle visible)
        let y = rng.gen_range(rad_val, 100.0);
        let (r, g, b) = rng.gen::<(f32, f32, f32)>(); // generate random (RGB) float tuple

        Circle {
            position: pt2(x, y),
            radius: rad_val,
            color: Rgba::new(r, g, b, 1.0),
        }
    }

    // set a new position
    pub fn set_position(&mut self, pos: Point2) {
        self.position = pos;
    }

    // get current position
    pub fn get_position(&self) -> Point2 {
        self.position
    }

    // actually draw the circle
    pub fn display(&self, draw: &Draw) {
        draw.ellipse() // use ellipse as primitive type
            .radius(self.radius) // set the radius
            .xy(self.position) // set the (drawing) position
            .color(self.color); // select a color
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_unit_circle() {
        let color = Rgba::new(0.0, 1.0, 0.5, 1.0);
        let c = Circle::new(color);

        assert_eq!(c.radius, 1.0);
        assert_eq!(c.position, pt2(0.0, 0.0));
        assert_eq!(c.color, color);
    }

    #[test]
    fn two_unequal_random_circles() {
        let c1 = Circle::random();
        let c2 = Circle::random();

        // circles should be different, but there is
        // a real possibilty of collision
        assert_ne!(c1.position, c2.position);
        assert_ne!(c1.radius, c2.radius);
        assert_ne!(c1.color, c2.color);
    }

    #[test]
    fn get_set_position() {
        let mut c = Circle::new(Rgba::new(0.0, 0.0, 0.0, 0.0));
        assert_eq!(c.position, pt2(0.0, 0.0));

        let test_pos = pt2(39.8, -45.7);
        c.set_position(test_pos);
        let current_pos = c.get_position();
        assert_eq!(current_pos[0], test_pos[0]);
    }
}
