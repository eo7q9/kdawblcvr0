/// A module to model a simple 2D ball.
extern crate rand;
use rand::Rng;
extern crate nannou;
use crate::circle::Circle;
use nannou::prelude::*;
use nannou_osc as osc; // handle Open Sound Control
use std::io::Result; // use "circle" module of current crate

mod ball;
use ball::Ball;

/// A ball which handles OSC data.
pub struct OscBall {
    ball: Ball,
    frequency: f32,
    sender: Option<osc::Sender<osc::Connected>>,
}

impl OscBall {
    // Create a white unit ball with no velocity.
    // The parameter has to contain the target host
    // (where pd is running) and the UDP port.
    #[allow(dead_code)]
    pub fn new(target: &str) -> OscBall {
        let socket = osc::sender()
            .expect("could not bind to default OSC sender socket")
            .connect(target)
            .expect("could not connect to target");
        OscBall {
            ball: Ball::new(),
            frequency: 0.0,
            sender: Some(socket),
        }
    }

    // Create PdBall with random values, except
    // for the message target.
    pub fn random(target: &str) -> PdBall {
        let mut rng = rand::thread_rng();
        PdBall {
            ball: Ball::random(),
            frequency: rng.gen_range(100.0, 1000.0),
            pdsend: fudi_rs::NetSendUdp::new(target),
        }
    }

    // Retrieve current velocity.
    pub fn get_velocity(&self) -> Point2 {
        self.ball.get_velocity()
    }

    // Set the velocity via a vector
    pub fn set_velocity(&mut self, v: Point2) {
        self.ball.set_velocity(v);
    }

    // Set radius of the ball
    #[allow(dead_code)]
    pub fn set_radius(&mut self, r: f32) {
        self.ball.set_radius(r);
    }

    // Get radius of the ball
    pub fn get_radius(&self) -> f32 {
        self.ball.get_radius()
    }

    // Set position of the ball
    pub fn set_position(&mut self, p: Point2) {
        self.ball.set_position(p);
    }

    // Get position of the ball
    pub fn get_position(&self) -> Point2 {
        self.ball.get_position()
    }

    // Set color of the ball
    #[allow(dead_code)]
    pub fn set_color(&mut self, c: Rgba) {
        self.ball.set_color(c);
    }

    // Get color of the ball
    #[allow(dead_code)]
    pub fn get_color(&self) -> Rgba {
        self.ball.get_color()
    }

    // Retrieve current frequency associated with the ball.
    #[allow(dead_code)]
    pub fn get_frequency(&self) -> f32 {
        self.frequency
    }

    // Associate a new frequency with the ball
    #[allow(dead_code)]
    pub fn set_frequency(&mut self, freq: f32) {
        self.frequency = freq;
    }

    // Send message to pure data.
    pub fn send(&self) {
        let msg = fudi_rs::PdMessage::Float(self.frequency);
        let osc_addr = "/foo".to_string();
        let osc_args = vec![];
        let packet = (osc_addr, osc_args);
        match self.sender {
            Some(s) => s.send(packet).ok(), // TODO: handle error more gracefully
            None => panic!("no OSC target"),
        }
    }

    // Draw the ball
    pub fn display(&self, draw: &Draw) {
        self.ball.display(draw);
    }
}

#[cfg(test)]
mod oscball_test {
    use super::*;

    #[test]
    fn random() {
        let b1 = OscBall::random("127.0.0.2:2345");
        let b2 = OscBall::random("127.0.0.2:2345");

        assert_ne!(b1.get_frequency(), b2.get_frequency());
    }

    #[test]
    fn velocity() {
        let mut b = OscBall::new("127.0.0.2:2345");
        assert_eq!(b.get_velocity(), pt2(0.0, 0.0));

        let v_new = pt2(0.5, 6.9);
        b.set_velocity(v_new);
        assert_eq!(b.get_velocity(), v_new);
    }

    #[test]
    fn radius() {
        let mut b = OscBall::new("127.0.0.2:2345");
        assert_eq!(b.get_radius(), 1.0);

        b.set_radius(3.5);
        assert_eq!(b.get_radius(), 3.5);
    }

    #[test]
    fn position() {
        let mut b = OscBall::new("127.0.0.2:2345");
        assert_eq!(b.get_position(), pt2(0.0, 0.0));

        let p_new = pt2(0.0, 0.0);
        b.set_position(p_new);
        assert_eq!(b.get_position(), p_new);
    }

    #[test]
    fn frequency() {
        let mut b = OscBall::new("127.0.0.2:2345");
        assert_eq!(b.get_frequency(), 0.0);

        let f_new = 83.7;
        b.set_frequency(f_new);
        assert_eq!(b.get_frequency(), f_new);
    }

    #[test]
    fn color() {
        let mut b = OscBall::new("127.0.0.2:2345");
        assert_eq!(b.get_color(), Rgba::new(1.0, 1.0, 1.0, 1.0));

        let c_new = Rgba::new(0.0, 0.4, 0.6, 1.0);
        b.set_color(c_new);
        assert_eq!(b.get_color(), c_new);
    }
}
