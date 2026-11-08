mod common;

use common::{Harness, run};
use crossterm::event::{KeyCode, KeyModifiers};
use drover::graphics::host_image_id;

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
        h.until("the placement hidden", |h| {
            h.screen();
            h.host_output().contains(&format!("\x1b_Ga=d,d=i,i={id},"))
        })
        .await;
        h.press(KeyCode::Esc, KeyModifiers::NONE).await;
        h.until("the placement back", |h| {
            h.screen();
            h.host_output().matches(&place).count() == 2
        })
        .await;
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
