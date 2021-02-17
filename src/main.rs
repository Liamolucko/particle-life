use particle_life::app;
use quicksilver::geom::Vector;

fn main() {
    quicksilver::run(
        quicksilver::Settings {
            title: "Particle Life",
            size: Vector {
                x: 1600.0,
                y: 900.0,
            },
            multisampling: Some(4),
            resizable: true,
            vsync: true,
            ..Default::default()
        },
        app,
    );
}
