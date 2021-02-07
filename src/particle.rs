use quicksilver::geom::Vector;

#[derive(Debug, Clone)]
pub struct Particle {
    pub pos: Vector,
    pub vel: Vector,
    pub r#type: usize,
}
