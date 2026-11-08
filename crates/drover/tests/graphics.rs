mod common;

use common::{Harness, run};
use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEventKind};
use drover::graphics::host_image_id;
use ratatui::layout::{Position, Rect};

const PNG_2X2: &str = "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAAEUlEQVR4nGP4z8AAQv8ZYAwAQ84H+VjtZqAAAAAASUVORK5CYII=";

fn show_image() -> String {
    format!("printf '\\033_Ga=T,f=100,i=5,c=4,r=2,q=2;{PNG_2X2}\\033\\\\'; echo\r")
}

fn placement_at(h: &Harness, address: &str) -> Option<(u16, u16)> {
    let hit = h
        .app()
        .hits
        .panes
        .iter()
        .find(|p| p.address.as_deref() == Some(address))?;
    let placement = *h
        .app()
        .panes
        .get(address)?
        .term
        .graphics()
        .placements
        .first()?;
    let col = u16::try_from(placement.col).ok()?;
    let row = u16::try_from(placement.row).ok()?;
    Some((hit.content.x + col, hit.content.y + row))
}

#[test]
fn kitty_images_pass_through_at_the_pane_offset() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        let address = h.focused_address().expect("a pane");
        h.typed(&show_image()).await;
        h.until("the image in the pane", |h| {
            placement_at(h, &address).is_some()
        })
        .await;
        let id = host_image_id(&address, 5);
        let upload = format!("\x1b_Ga=t,t=d,f=32,s=2,v=2,i={id},q=2,m=0;");
        h.until("the upload", |h| h.host_output().contains(&upload))
            .await;
        let (x, y) = placement_at(&h, &address).expect("a placement");
        let place = format!("\x1b[{};{}H\x1b_Ga=p,i={id},", y + 1, x + 1);
        h.until("the placement", |h| {
            h.screen();
            h.host_output().contains(&place)
        })
        .await;
        let out = h.host_output();
        let at = out.find(&place).expect("placed");
        assert!(out[..at].contains("\x1b7"), "the cursor is saved first");
        assert!(
            out[at..].contains(",c=4,r=2,z=0,C=1,q=2"),
            "{:?}",
            &out[at..]
        );
        assert!(out[at..].contains("\x1b8"), "the cursor comes back");
        assert_eq!(out.matches(&upload).count(), 1, "one upload");

        h.prefix_char('W').await;
        assert!(h.app().view.overlay.is_some(), "the rename dialog is open");
        h.screen();
        h.screen();
        assert!(
            !h.host_output().contains(&format!("\x1b_Ga=d,d=i,i={id},")),
            "a dialog away from the image leaves it alone"
        );
        h.press(KeyCode::Esc, KeyModifiers::NONE).await;
        h.screen();
        assert_eq!(h.host_output().matches(&place).count(), 1);
        assert_eq!(h.host_output().matches(&upload).count(), 1, "no re-upload");

        h.typed("printf '\\033_Ga=d,d=I,i=5,q=2\\033\\\\'\r").await;
        h.until("the image deleted", |h| {
            h.screen();
            h.host_output()
                .contains(&format!("\x1b_Ga=d,d=I,i={id},q=2;"))
        })
        .await;
    });
}

#[test]
fn disabled_graphics_send_nothing_to_the_host() {
    run(|| async {
        let mut h = Harness::with_config("[terminal]\nkitty_graphics = false\n").await;
        h.ready().await;
        let address = h.focused_address().expect("a pane");
        h.typed(&show_image()).await;
        h.typed("echo done-now\r").await;
        h.until("the shell ran on", |h| {
            h.pane_text(&address).contains("done-now")
        })
        .await;
        h.screen();
        assert!(!h.host_output().contains("\x1b_G"), "no kitty commands");
        assert!(placement_at(&h, &address).is_none());
    });
}

fn show_big_image(rows: u16) -> String {
    format!("printf '\\033_Ga=T,f=100,i=6,c=60,r={rows},C=1,q=2;{PNG_2X2}\\033\\\\'; echo\r")
}

async fn big_image(h: &mut Harness) -> Rect {
    tall_image(h, 20).await
}

async fn tall_image(h: &mut Harness, rows: u16) -> Rect {
    let address = h.focused_address().expect("a pane");
    h.typed(&show_big_image(rows)).await;
    h.until("the big image placed", |h| {
        h.screen();
        placement_at(h, &address).is_some() && !h.app().graphics.placed().is_empty()
    })
    .await;
    let (x, y) = placement_at(h, &address).expect("a placement");
    let content = h
        .app()
        .hits
        .panes
        .iter()
        .find(|p| p.address.as_deref() == Some(address.as_str()))
        .expect("the pane")
        .content;
    Rect::new(x, y, 60, rows).intersection(content)
}

fn pieces(h: &Harness) -> Vec<Rect> {
    h.app()
        .graphics
        .placed()
        .into_iter()
        .map(|p| Rect::new(p.x, p.y, p.cols, p.rows))
        .collect()
}

fn assert_cut(h: &Harness, image: Rect, covers: &[Rect]) {
    let shown = pieces(h);
    for y in image.top()..image.bottom() {
        for x in image.left()..image.right() {
            let at = Position::new(x, y);
            let covered = covers.iter().any(|c| c.contains(at));
            let placed = shown.iter().filter(|p| p.contains(at)).count();
            assert_eq!(
                placed,
                usize::from(!covered),
                "cell ({x}, {y}) covered={covered}: pieces {shown:?}"
            );
        }
    }
}

#[test]
fn overlays_cut_images_only_where_they_cover_them() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        let image = big_image(&mut h).await;
        assert_cut(&h, image, &[]);
        h.prefix_char('W').await;
        h.screen();
        let dialog = Rect::new((120 - 56) / 2, (40 - 7) / 2, 56, 7);
        assert!(h.app().hits.covers.contains(&dialog));
        assert!(dialog.intersects(image));
        assert_cut(&h, image, &[dialog]);
        h.press(KeyCode::Esc, KeyModifiers::NONE).await;
        h.screen();
        assert_cut(&h, image, &[]);
        assert_eq!(pieces(&h), [image]);
    });
}

#[test]
fn selections_cut_images_cell_by_cell() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        let image = big_image(&mut h).await;
        let (x, y) = (image.x + 5, image.y + 4);
        h.mouse(MouseEventKind::Down(MouseButton::Left), x, y).await;
        h.mouse(MouseEventKind::Drag(MouseButton::Left), x + 10, y + 2)
            .await;
        h.screen();
        let width = h
            .app()
            .hits
            .panes
            .first()
            .map(|p| p.content.width)
            .expect("a pane");
        let left = image.x;
        let covers = [
            Rect::new(x, y, left + width - x, 1),
            Rect::new(left, y + 1, width, 1),
            Rect::new(left, y + 2, x + 10 - left + 1, 1),
        ];
        assert_cut(&h, image, &covers);
    });
}

#[test]
fn copy_mode_search_cuts_images_at_the_match() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        let image = big_image(&mut h).await;
        h.typed("echo FIND$((1))ME\r").await;
        h.until("the output", |h| {
            h.screen()
                .lines()
                .any(|l| l.contains("FIND1ME") && !l.contains('$'))
        })
        .await;
        h.prefix_char('[').await;
        h.typed("/FIND1ME\r").await;
        let screen = h.screen();
        let (y, line) = screen
            .lines()
            .enumerate()
            .find(|(_, l)| l.contains("FIND1ME") && !l.contains('$'))
            .expect("the match on screen");
        let x = line[..line.find("FIND1ME").expect("found")].chars().count();
        let found = Rect::new(
            u16::try_from(x).expect("x"),
            u16::try_from(y).expect("y"),
            7,
            1,
        );
        assert!(found.intersects(image));
        let copy = h.app().view.copy.as_ref().expect("copy mode");
        let at = copy.mode.cursor();
        let pane = h.app().panes.get(&copy.address).expect("the pane");
        let top = pane.term.viewport_top();
        let content = h.app().hits.panes[0].content;
        let cursor = Rect::new(
            content.x + at.col,
            content.y + u16::try_from(at.row - top).expect("visible"),
            1,
            1,
        );
        assert_cut(&h, image, &[found, cursor]);
    });
}

#[test]
fn mode_bars_and_popups_cut_images() {
    run(|| async {
        let config =
            "[[keys.command]]\nkey = \"prefix+y\"\ntype = \"popup\"\ncommand = \"sleep 600\"\n";
        let mut h = Harness::with_config(config).await;
        h.ready().await;
        let image = tall_image(&mut h, 60).await;
        let surface = h.app().regions.surface;
        assert_eq!(image.bottom(), surface.bottom());
        h.press(KeyCode::Char('b'), KeyModifiers::CONTROL).await;
        h.screen();
        let bar = Rect::new(surface.x, surface.bottom() - 1, surface.width, 1);
        assert_cut(&h, image, &[bar]);
        h.press(KeyCode::Char('y'), KeyModifiers::NONE).await;
        h.until("the popup", |h| {
            h.screen();
            h.app().popup.as_ref().is_some_and(|p| p.pane.is_some())
        })
        .await;
        let outer = h
            .app()
            .popup
            .as_ref()
            .and_then(|p| p.geometry)
            .expect("a popup")
            .outer;
        assert!(outer.intersects(image));
        assert_cut(&h, image, &[outer]);
    });
}
