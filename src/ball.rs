/// A module to model a simple 2D ball.
extern crate rand;
use rand::Rng;
extern crate nannou;
use nannou::prelude::*;
use nannou_osc as osc; // handle Open Sound Control

use crate::circle::Circle;
use std::io::Result; // use "circle" module of current crate

/// A (2D) ball is a circle with a velocity vector.
pub struct Ball {
    circle: Circle,
    velocity: Point2,
}

impl Ball {
    // Create a white unit ball with no velocity.
    pub fn new() -> Ball {
        Ball {
            circle: Circle::new(Rgba::new(1.0, 1.0, 1.0, 1.0)),
            velocity: pt2(0.0, 0.0),
        }
    }

    // Generate a ball with random values.
    pub fn random() -> Ball {
        let mut rng = rand::thread_rng();
        let x = rng.gen_range(-10.0, 10.0);
        let y = rng.gen_range(-10.0, 10.0);
        Ball {
            circle: Circle::random(),
            velocity: pt2(x, y),
        }
    }

    // Retrieve current velocity.
    pub fn get_velocity(&self) -> Point2 {
        self.velocity.clone()
    }

    // Set the velocity via a vector
    pub fn set_velocity(&mut self, v: Point2) {
        self.velocity = v;
    }

    // Generate a ball with random values.
    pub fn randomise_velocity(&mut self) {
        let mut rng = rand::thread_rng();
        let x = rng.gen_range(-10.0, 10.0);
        let y = rng.gen_range(-10.0, 10.0);
        self.set_velocity(pt2(x, y));
    }

    // Set radius of the ball
    pub fn set_radius(&mut self, r: f32) {
        self.circle.radius = r;
    }

    // Get radius of the ball
    pub fn get_radius(&self) -> f32 {
        self.circle.radius.clone()
    }

    // Set position of the ball
    pub fn set_position(&mut self, p: Point2) {
        self.circle.position = p;
    }

    // Get position of the ball
    pub fn get_position(&self) -> Point2 {
        self.circle.position.clone()
    }

    // Set color of the ball
    pub fn set_color(&mut self, c: Rgba) {
        self.circle.color = c;
    }

    // Get color of the ball
    pub fn get_color(&self) -> Rgba {
        self.circle.color.clone()
    }

    // Draw the ball
    pub fn display(&self, draw: &Draw) {
        self.circle.display(draw);
    }
}

#[cfg(test)]
mod ball_test {
    use super::*;

    #[test]
    fn new_ball() {
        let b = Ball::new();

        assert_eq!(b.circle.radius, 1.0);
        assert_eq!(b.circle.color, Rgba::new(1.0, 1.0, 1.0, 1.0));
        assert_eq!(b.circle.position, pt2(0.0, 0.0));
        assert_eq!(b.velocity, pt2(0.0, 0.0));
    }

    #[test]
    fn random() {
        let b1 = Ball::random();
        let b2 = Ball::random();

        assert_ne!(b1.get_velocity(), b2.get_velocity());
    }

    #[test]
    fn velocity() {
        let mut b = Ball::new();
        assert_eq!(b.get_velocity(), pt2(0.0, 0.0));

        let v_new = pt2(0.5, 6.9);
        b.set_velocity(v_new);
        assert_eq!(b.get_velocity(), v_new);
    }

    #[test]
    fn radius() {
        let mut b = Ball::new();
        assert_eq!(b.get_radius(), 1.0);

        b.set_radius(3.5);
        assert_eq!(b.get_radius(), 3.5);
    }

    #[test]
    fn position() {
        let mut b = Ball::new();
        assert_eq!(b.get_position(), pt2(0.0, 0.0));

        let p_new = pt2(0.0, 0.0);
        b.set_position(p_new);
        assert_eq!(b.get_position(), p_new);
    }

    #[test]
    fn color() {
        let mut b = Ball::new();
        assert_eq!(b.get_color(), Rgba::new(1.0, 1.0, 1.0, 1.0));

        let c_new = Rgba::new(0.0, 0.4, 0.6, 1.0);
        b.set_color(c_new);
        assert_eq!(b.get_color(), c_new);
    }
}

/// A ball which interacts with pure data.
pub struct PdBall {
    ball: Ball,
    frequency: f32,
    pdsend: fudi_rs::NetSendUdp,
}

impl PdBall {
    // Create a white unit ball with no velocity.
    // The parameter has to contain the target host
    // (where pd is running) and the UDP port.
    #[allow(dead_code)]
    pub fn new(target: &str) -> PdBall {
        PdBall {
            ball: Ball::new(),
            frequency: 0.0,
            pdsend: fudi_rs::NetSendUdp::new(target),
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
    pub fn send(&self) -> Result<usize> {
        let msg = fudi_rs::PdMessage::Float(self.frequency);
        self.pdsend.send(&msg)
    }

    // Draw the ball
    pub fn display(&self, draw: &Draw) {
        self.ball.display(draw);
    }
}

#[cfg(test)]
mod pdball_test {
    use super::*;

    #[test]
    fn random() {
        let b1 = PdBall::random("127.0.0.2:2345");
        let b2 = PdBall::random("127.0.0.2:2345");

        assert_ne!(b1.get_frequency(), b2.get_frequency());
    }

    #[test]
    fn velocity() {
        let mut b = PdBall::new("127.0.0.2:2345");
        assert_eq!(b.get_velocity(), pt2(0.0, 0.0));

        let v_new = pt2(0.5, 6.9);
        b.set_velocity(v_new);
        assert_eq!(b.get_velocity(), v_new);
    }

    #[test]
    fn radius() {
        let mut b = PdBall::new("127.0.0.2:2345");
        assert_eq!(b.get_radius(), 1.0);

        b.set_radius(3.5);
        assert_eq!(b.get_radius(), 3.5);
    }

    #[test]
    fn position() {
        let mut b = PdBall::new("127.0.0.2:2345");
        assert_eq!(b.get_position(), pt2(0.0, 0.0));

        let p_new = pt2(0.0, 0.0);
        b.set_position(p_new);
        assert_eq!(b.get_position(), p_new);
    }

    #[test]
    fn frequency() {
        let mut b = PdBall::new("127.0.0.2:2345");
        assert_eq!(b.get_frequency(), 0.0);

        let f_new = 83.7;
        b.set_frequency(f_new);
        assert_eq!(b.get_frequency(), f_new);
    }

    #[test]
    fn color() {
        let mut b = PdBall::new("127.0.0.2:2345");
        assert_eq!(b.get_color(), Rgba::new(1.0, 1.0, 1.0, 1.0));

        let c_new = Rgba::new(0.0, 0.4, 0.6, 1.0);
        b.set_color(c_new);
        assert_eq!(b.get_color(), c_new);
    }
}

/// A ball which handles OSC data.
pub struct OscBall {
    ball: Ball,
    address: String,
    arguments: Vec<osc::Type>,
    sender: Option<osc::Sender<osc::Connected>>,
}

impl OscBall {
    // Create a white unit ball with no velocity.
    // The parameter has to contain the target host
    // (where OSC commands can be received) and
    // the port.
    #[allow(dead_code)]
    pub fn new(target: &str) -> OscBall {
        let socket = osc::sender()
            .expect("could not bind to default OSC sender socket")
            .connect(target)
            .expect("could not connect to target");
        OscBall {
            ball: Ball::new(),
            address: "".to_string(),
            arguments: vec![],
            sender: Some(socket),
        }
    }

    // Create OscBall with random core values.
    // OSC address and arguments are empty.
    pub fn random(target: &str) -> OscBall {
        let rball = Ball::random();
        let mut oball = OscBall::new(target);
        oball.set_color(rball.get_color());
        oball.set_position(rball.get_position());
        oball.set_radius(rball.get_radius());
        oball.set_velocity(rball.get_velocity());
        return oball;
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
    pub fn set_color(&mut self, c: Rgba) {
        self.ball.set_color(c);
    }

    // Get color of the ball
    pub fn get_color(&self) -> Rgba {
        self.ball.get_color()
    }

    // Set the OSC address.
    pub fn set_address(&mut self, address: String) {
        self.address = address;
    }

    // Retrieve current OSC address in use.
    pub fn get_address(&self) -> String {
        return self.address.clone();
    }

    // Set the OSC arguments.
    pub fn set_arguments(&mut self, arguments: Vec<osc::Type>) {
        self.arguments = arguments;
    }

    // Retrieve current OSC arguments in use.
    pub fn get_arguments(&self) -> Vec<osc::Type> {
        return self.arguments.clone();
    }

    // Send message to pure data.
    pub fn send(&self) {
        let osc_args = vec![];
        let packet = (&self.address, osc_args);
        match &self.sender {
            Some(s) => {
                s.send(packet).ok(); // TODO: handle error more gracefully
                return;
            }
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

        assert_ne!(b1.get_color(), b2.get_color());
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
    fn color() {
        let mut b = OscBall::new("127.0.0.2:2345");
        assert_eq!(b.get_color(), Rgba::new(1.0, 1.0, 1.0, 1.0));

        let c_new = Rgba::new(0.0, 0.4, 0.6, 1.0);
        b.set_color(c_new);
        assert_eq!(b.get_color(), c_new);
    }

    #[test]
    fn address() {
        let mut b = OscBall::new("127.0.0.2:2345");
        let t_addr = "/foo".to_string();

        b.set_address(t_addr.clone());
        assert_eq!(b.get_address(), t_addr);
    }

    #[test]
    fn arguments() {
        let mut b = OscBall::new("127.0.0.2:2345");
        let t_args = vec![osc::Type::Float(2.3)];

        b.set_arguments(t_args.clone());
        assert_eq!(b.get_arguments(), t_args);
    }
}
