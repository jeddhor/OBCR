//! Smoke test: renders a map position to PNG with a provisional tile mapping.
//! Usage: cargo run -p obc-render --example shot -- MAP X Y ANGLE OUT.png [WIDTH HEIGHT FOVWIDTH]
use obc_formats::DataDir;
use obc_formats::exe::MzExe;
use obc_formats::maps::{Compression, MapArchive};
use obc_formats::vswap::{PageFile, Shape};
use obc_render::Framebuffer;
use obc_render::view3d::{Cell, FloorCeiling, Scene, Textures, View3D, ViewConfig};

struct Tex {
    walls: Vec<Vec<u8>>,
    sprites: Vec<Shape>,
}
impl Textures for Tex {
    fn wall(&self, p: u16) -> Option<&[u8]> {
        self.walls.get(p as usize).map(Vec::as_slice)
    }
    fn sprite(&self, i: u16) -> Option<&Shape> {
        self.sprites.get(i as usize)
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let map: usize = a[1].parse().unwrap();
    let (x, y, ang): (f64, f64, i32) =
        (a[2].parse().unwrap(), a[3].parse().unwrap(), a[4].parse().unwrap());
    let (w, h, fov) = if a.len() > 8 {
        (a[6].parse().unwrap(), a[7].parse().unwrap(), a[8].parse().unwrap())
    } else {
        (304, 152, 304)
    };
    let d = DataDir::open(obc_formats::datadir::discover(None, None).unwrap()).unwrap();
    let pf = PageFile::parse(d.read("GFXTILES.BC").unwrap()).unwrap();
    let tex = Tex {
        walls: (0..pf.num_walls()).map(|i| pf.wall(i).unwrap().to_vec()).collect(),
        sprites: (0..pf.num_sprites()).map(|i| pf.sprite(i).unwrap()).collect(),
    };
    let maps = MapArchive::parse(
        &d.read("MAPHEAD.BC").unwrap(),
        d.read("MAPTEMP.BC").unwrap(),
        Compression::Rlew,
    )
    .unwrap();
    let m = maps.map(map).unwrap().unwrap();
    let mut cells = vec![Cell::Empty; 4096];
    for ty in 0..64 {
        for tx in 0..64 {
            let v = m.tile(0, tx, ty);
            if (1..=136).contains(&v) {
                cells[tx * 64 + ty] = Cell::Wall { horiz: v - 1, vert: v - 1 };
            }
        }
    }
    let pal = MzExe::parse(d.read("COUNT.EXE").unwrap()).unwrap().palette(0x2DDB0).unwrap();
    let mut view =
        View3D::new(ViewConfig { width: w, height: h, fov_width: fov, classic: w == fov });
    let mut fb = Framebuffer::new(w, h);
    let scene = Scene {
        cells: &cells,
        pwall_pos: 0,
        jamb: (0, 0),
        floor_ceiling: FloorCeiling::Solid { ceiling: 0x1d, floor: 0x19 },
        player_x: (x * 65536.0) as i32,
        player_y: (y * 65536.0) as i32,
        player_angle: ang,
        sprites: &[],
        weapon: Some(600),
    };
    view.render(&mut fb, 0, 0, &scene, &tex);
    let mut rgba = Vec::new();
    obc_render::palette::to_rgba(&fb.pixels, &pal, &mut rgba);
    let f = std::io::BufWriter::new(std::fs::File::create(&a[5]).unwrap());
    let mut e = png::Encoder::new(f, w as u32, h as u32);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    e.write_header().unwrap().write_image_data(&rgba).unwrap();
}
