mod common;

use common::term;
use vt::{ImageFormat, Options, Placement, Terminal};

const PNG_1X1: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==";
const RGB_2X1: &str = "/wAAAP8A";
const NONE: [Placement; 0] = [];

fn apc(control: &str, payload: &str) -> Vec<u8> {
    format!("\x1b_G{control};{payload}\x1b\\").into_bytes()
}

#[test]
fn answers_support_queries() {
    let mut t = term(20, 5);
    t.feed(&apc("i=31,s=1,v=1,a=q,t=d,f=24", "AAAA"));
    assert_eq!(t.take_replies(), b"\x1b_Gi=31;OK\x1b\\");
    t.feed(&apc("i=32,a=q,t=d,f=100", PNG_1X1));
    assert_eq!(t.take_replies(), b"\x1b_Gi=32;OK\x1b\\");
    assert_eq!(t.graphics().placements, NONE);
}

#[test]
fn file_media_are_refused() {
    let mut t = term(20, 5);
    t.feed(&apc("i=34,a=q,t=f,f=100", "L2V0Yy9wYXNzd2Q="));
    let reply = String::from_utf8(t.take_replies()).unwrap();
    assert!(reply.starts_with("\x1b_Gi=34;E"), "{reply:?}");
}

#[test]
fn quiet_flag_suppresses_replies() {
    let mut t = term(20, 5);
    t.feed(&apc("i=35,a=T,f=24,s=2,v=1,q=2", RGB_2X1));
    assert_eq!(t.take_replies(), b"");
    assert_eq!(t.graphics().placements.len(), 1);
}

#[test]
fn placements_expose_position_size_and_data() {
    let mut t = term(20, 5);
    t.feed(b"ab\r\ncd");
    t.feed(&apc("i=7,p=3,a=T,f=24,s=2,v=1,c=4,r=2,z=-5", RGB_2X1));
    assert_eq!(t.take_replies(), b"\x1b_Gi=7,p=3;OK\x1b\\");
    let graphics = t.graphics();
    assert!(graphics.generation > 0);
    let placement = graphics.placements[0];
    assert_eq!(
        placement,
        Placement {
            image: 7,
            placement: 3,
            image_generation: placement.image_generation,
            z: -5,
            col: 2,
            row: 1,
            cols: 4,
            rows: 2,
            width: 40,
            height: 40,
            x_offset: 0,
            y_offset: 0,
            source_x: 0,
            source_y: 0,
            source_width: 2,
            source_height: 1,
        }
    );
    let image = t.image(7).unwrap();
    assert_eq!((image.width, image.height), (2, 1));
    assert_eq!(image.format, ImageFormat::Rgb);
    assert_eq!(image.data, [255, 0, 0, 0, 255, 0]);
    assert_eq!(image.generation, placement.image_generation);
}

#[test]
fn png_images_are_decoded() {
    let mut t = term(20, 5);
    t.feed(&apc("i=9,a=T,f=100,q=1", PNG_1X1));
    let image = t.image(9).unwrap();
    assert_eq!((image.width, image.height), (1, 1));
    assert_eq!(image.format, ImageFormat::Rgba);
    assert_eq!(image.data.len(), 4);
}

#[test]
fn placements_move_with_scrolling() {
    let mut t = term(20, 4);
    t.feed(&apc("i=1,a=T,f=24,s=2,v=1,r=2,C=1,q=2", RGB_2X1));
    assert_eq!(t.graphics().placements[0].row, 0);
    t.feed(b"\r\n\r\n\r\n\r\n");
    assert_eq!(t.graphics().placements[0].row, -1);
    t.feed(b"\r\n");
    assert_eq!(t.graphics().placements, NONE);
    t.scroll(-2);
    assert_eq!(t.graphics().placements[0].row, 0);
}

#[test]
fn deletes_and_virtual_placements_are_not_drawn() {
    let mut t = term(20, 4);
    t.feed(&apc("i=1,a=T,f=24,s=2,v=1,q=2", RGB_2X1));
    t.feed(&apc("a=d,d=A,q=2", ""));
    assert_eq!(t.graphics().placements, NONE);
    t.feed(&apc("i=2,a=T,U=1,f=24,s=2,v=1,c=2,r=1,q=2", RGB_2X1));
    assert_eq!(t.graphics().placements, NONE);
    assert!(t.image(2).is_some());
}

#[test]
fn zero_storage_disables_images() {
    let mut t = Terminal::new(
        20,
        4,
        Options {
            image_bytes: 0,
            ..Options::default()
        },
    )
    .unwrap();
    t.feed(&apc("i=31,s=1,v=1,a=q,t=d,f=24", "AAAA"));
    assert_eq!(t.take_replies(), b"");
    assert_eq!(t.graphics().placements, NONE);
}

#[test]
fn image_data_split_across_feeds() {
    let mut t = term(20, 4);
    let bytes = apc("i=4,a=T,f=24,s=2,v=1,q=2", RGB_2X1);
    let (a, b) = bytes.split_at(9);
    t.feed(a);
    t.feed(b);
    assert_eq!(t.graphics().placements.len(), 1);
    assert_eq!(t.row_text(t.viewport_top()).as_deref(), Some(""));
}
