//! The loading screens: the front end's and the race's.

use crate::*;

impl App {
    /// The sprites of 0x4DDA00(`step`)'s frame (`nfs_game::layout::loading`), the packs loaded
    /// on first use with `LANGUAGE` (`[0x657C84]`, `App::language`) choosing the caption.
    pub(crate) fn fe_loading_sprites(&mut self, step: u32) -> Vec<Sprite> {
        let Some(r) = self.renderer.as_mut() else { return Vec::new() };
        if self.fe.loading.is_none() {
            let language = u32::from(self.language);
            self.fe.loading = Some(nfs_game::layout::loading::LoadingScreen::load(r, &self.paths, language));
        }
        let aspect = r.aspect();
        self.fe.loading.as_ref().map(|l| l.sprites(aspect, step)).unwrap_or_default()
    }

    /// 0x4A4A70's frame as sprites for this platform's target. The original copies the 640×480
    /// surface 1:1 into the middle of the race mode (0x53D570); a browser's canvas is the viewport
    /// instead of that 640×480 mode, so there the same picture is fitted to the logical size the way
    /// the front end's 640×480 screens are (`RaceLoading::sprites_fitted`) — a platform mapping.
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn race_loading_sprites(&self, lw: u32, lh: u32) -> Vec<Sprite> {
        self.fe.race_loading.as_ref().map(|l| l.sprites_fitted(lw as f32, lh as f32)).unwrap_or_default()
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn race_loading_sprites(&self, lw: u32, lh: u32) -> Vec<Sprite> {
        self.fe.race_loading.as_ref().map(|l| l.sprites(lw as f32, lh as f32)).unwrap_or_default()
    }

    /// **0x4DDA00(`step`)** — one whole loading-screen frame, drawn and presented at once, as
    /// 0x4D3420 does between the stages of its load (`controls.md` §4.165.3). It first stops the
    /// music (`MusicStop(1)`, 0x4DDA16). The port's own front-end load has no stages in 0x4D3420's
    /// order, so only the first (0) and last (9) frames are drawn — PORT-DEVIATION D31: steps 1…8,
    /// which grow the bar 13.78 pixels each, are not. As in the original the screen stays up
    /// only while the load runs; nothing holds it longer.
    /// What 0x4A4A70 reads: `Game.GameType` (the launch's), the career era `0x5032D0`, the
    /// track, `MIRROR` / `REVERSE`, a network session and `LANGUAGE`. The session tests are the same
    /// fields as 0x4D4072's (read at 0x4A4AD4…0x4A4B00 for the progress send and at 0x4A4F5F…0x4A4F9B for
    /// the `MLoading.fsh` draw: `[0x628C70]`, `+8`, `+0xBF == 0`, `+0xBE`, `+0xDC ≥ 0`), which is
    /// `NetPlay::established`.
    fn race_loading_inputs(
        &self,
        game_type: u8,
        track: &str,
        mirror: bool,
        reverse: bool,
    ) -> nfs_game::layout::loading::RaceLoadingInputs {
        use nfs_formats::career::Era;
        nfs_game::layout::loading::RaceLoadingInputs {
            game_type,
            era: self.career.as_ref().map_or(0, |c| match c.career.era {
                Era::Classic => 1,
                Era::Golden => 2,
                Era::Modern => 3,
            }),
            track: track.to_string(),
            mirror,
            reverse,
            network: self.net.session.as_ref().is_some_and(|n| n.established()),
            language: u32::from(self.language),
            rows: Vec::new(),
        }
    }

    /// One call of **0x4A4A70(`step`)** into the surface `RaceLoading` keeps.
    pub(crate) fn race_loading_step(&mut self, r: &mut Renderer, step: i32) -> bool {
        let game_type = self.fe.race_game_type.unwrap_or(self.fe.game_type);
        let mut input = self.race_loading_inputs(game_type, &self.track.name, self.mirror, self.backwards);
        // 0x4A4AD4…0x4A4B4F: the progress record, while the screen is up (`[0x655A28]`, set by step
        // 0 and cleared by step 10), and 0x4A5149…0x4A5353: every player's row, on every call
        let active = step == 0 || (step != 10 && self.fe.race_loading.as_ref().is_some_and(|l| l.active));
        if active && let Some(n) = self.net.session.as_mut().filter(|n| n.in_race != 0) {
            n.send_loading_step(step);
            input.rows = n.loading_rows(r);
        }
        let rl = self.fe.race_loading.get_or_insert_with(|| nfs_game::layout::loading::RaceLoading::new(r));
        rl.step(r, &self.paths, step, &input)
    }

    /// **0x4A4A70(`step`)**, drawn and presented at once — the loading screen between the front
    /// end and the race (`controls.md` §4.167). The port's race load is `start_race`, whose
    /// stages are not 0x4B67B0's, so each step is drawn where the port's work of the original's
    /// step has finished, in the original's order (NM, 2026-09-30): 0 as it starts (main's 0x4B6CCA),
    /// 1 and 2 after the picture post (0x427A60), 3 after `TrackWorld::upload` (0x469065, the track
    /// data), 4 and 5 after the sky (the end of 0x468FB0), 6 after the camera data (0x471B70, moved up
    /// for the order), 7 after the collision sounds (0x4A8CD0), 8 before and 9 after the cars
    /// (0x414E63, 0x414E74), 10 after the barrier (0x4B68CB). PORT-DEVIATION D31: the *work* between two
    /// steps is the port's, not 0x4B67B0's — the bar moves when the port's equivalent finishes.
    pub(crate) fn race_loading_frame(&mut self, r: &mut Renderer, step: i32) {
        tracing::debug!(target: "loading", step, "0x4A4A70");
        if !self.race_loading_step(r, step) {
            return;
        }
        let (lw, lh) = self.hud_logical_on(r);
        let sprites = self.race_loading_sprites(lw, lh);
        let scene = Scene { sprites, clear_color: [0.0, 0.0, 0.0, 1.0], ..Scene::default() };
        if let Err(e) = r.render(&scene, &self.camera) {
            tracing::error!(error = %e, "race loading screen render failed");
        }
        // **TEST GLUE** (not in the original): `NFS_LOADING_SHOT=<path>` writes the loading screen of
        // step 9 as a still after `NFS_LOADING_WAIT` ms (default 3000) of draining the session — the
        // other machine's `0x256` records and its picture arrive meanwhile — redrawn with step −1
        // (0x4A4A70 with −1 draws the rows again and leaves the bar as it was)
        if step == 9
            && let Some(path) = nfs_game::config::cfg().nfs_loading_shot.as_str()
        {
            let wait = nfs_game::config::cfg().nfs_loading_wait.as_str().and_then(|v| v.parse::<u64>().ok()).unwrap_or(3000);
            let until = Instant::now() + Duration::from_millis(wait);
            while Instant::now() < until {
                if let Some(n) = self.net.session.as_mut() {
                    let _ = n.poll_lobby();
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            self.race_loading_step(r, -1);
            let sprites = self.race_loading_sprites(lw, lh);
            let scene = Scene { sprites, clear_color: [0.0, 0.0, 0.0, 1.0], ..Scene::default() };
            match r.render_to_image(&scene, &self.camera, shot_geometry().0, shot_geometry().1) {
                Ok(rgba) => {
                    let _ = image::save_buffer(&path, &rgba, shot_geometry().0, shot_geometry().1, image::ColorType::Rgba8);
                    tracing::info!(path, "loading screen still written (NFS_LOADING_SHOT)");
                }
                Err(e) => tracing::error!(error = %e, "loading still failed"),
            }
        }
    }

    pub(crate) fn fe_loading_frame(&mut self, step: u32) {
        // 0x4DDA00: `MusicStop(1)` (0x4DDA14) — a fade of one step, then the update stops it
        self.fe_music(|m, a, _| m.stop_fade(a, 1));
        let sprites = self.fe_loading_sprites(step);
        let scene = Scene { sprites, clear_color: [0.0, 0.0, 0.0, 1.0], ..Scene::default() };
        if let Some(r) = self.renderer.as_mut()
            && let Err(e) = r.render(&scene, &self.camera)
        {
            tracing::error!(error = %e, "loading screen render failed");
        }
    }
}

/// **Browser only.** The race loading screen (0x4A4A70) put up while the shell downloads the
/// launch's files. The original draws steps 0…5 around its synchronous track load; in a browser
/// that load is mostly the shell's download, so the bar moves with the files prepared so far
/// (steps 0…4) and step 5 — the picture of the track itself — is shown once they are all here.
/// The screen is drawn from the frame loop (`draw_frame`), not from `start_race`, so it is on the
/// canvas while the download runs, before the launch's synchronous part blocks the main thread.
#[cfg(target_arch = "wasm32")]
pub(crate) struct WebTrackLoading {
    /// what 0x4A4A70 reads — the game type, era, track and direction of the launch
    inputs: nfs_game::layout::loading::RaceLoadingInputs,
    /// the shell's download finished: the frame after this is the last one before the launch
    ready: bool,
    /// the step last put on the surface, so an unchanged frame is not drawn again
    drawn: i32,
    /// the ready frame (step 5) was drawn and the launch is asked for
    resume_requested: bool,
}

#[cfg(target_arch = "wasm32")]
impl App {
    /// Start the browser's track preparation on the game's own loading screen. The shell's HTML
    /// panel is hidden from the first frame on (`nfsTrackScreen`) and comes back only for its
    /// Retry / error report; this runs before any career or series state is changed, as the old
    /// gate did.
    pub(crate) fn web_track_loading_begin(&mut self, track: &str) {
        let ready = crate::web::prepare_track(track);
        // the launch's direction: a network race's host options, else the menu's rows (a series
        // round decides its own below the gate — the difference is the step-5 picture only)
        let (mirror, reverse) = self
            .net
            .race
            .as_ref()
            .map_or_else(
                || self.fe.menu.as_ref().map_or((false, false), |m| (m.mirror, m.backwards)),
                |r| (r.mirror, r.backwards),
            );
        self.fe.web_track_loading = Some(WebTrackLoading {
            inputs: self.race_loading_inputs(self.fe.game_type, track, mirror, reverse),
            ready,
            drawn: -1,
            resume_requested: false,
        });
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    /// The launch was asked for again: `true` when the files are here and `start_from_layout` may
    /// go on, `false` while the download runs or while the ready frame still has to be drawn.
    ///
    /// The ready frame (step 5) gets a frame of its own: the launch's synchronous load is one long
    /// JavaScript task, and a frame presented inside it is never shown. Drawing step 5 in the
    /// previous frame loop turn keeps it on the canvas for the whole load.
    pub(crate) fn web_track_loading_resume(&mut self) -> bool {
        let Some(state) = self.fe.web_track_loading.as_mut() else { return true };
        if state.ready {
            self.fe.web_track_loading = None;
            crate::web::track_screen(false);
            return true;
        }
        if crate::web::loading() {
            // a retry from somewhere else while the download is still running: keep waiting
            return false;
        }
        state.ready = true;
        state.drawn = -1;
        state.resume_requested = false;
        if let Some(window) = &self.window {
            window.request_redraw();
        }
        false
    }

    /// One frame of the browser's track loading screen: steps 0…4 follow the shell's progress, step
    /// 5 the finished download. It hides the shell's panel once the game's frame is up and asks for
    /// the deferred launch after the ready frame has been presented.
    pub(crate) fn web_track_loading_frame(&mut self) {
        let Some((step, drawn)) = self.fe.web_track_loading.as_ref().map(|s| {
            let step = if s.ready { 5 } else { ((crate::web::track_progress() * 5.0) as i32).min(4) };
            (step, s.drawn)
        }) else {
            return;
        };
        let inputs = self.fe.web_track_loading.as_ref().map(|s| s.inputs.clone()).expect("checked above");
        if step != drawn {
            let Some(mut r) = self.renderer.take() else { return };
            let rl = self.fe.race_loading.get_or_insert_with(|| nfs_game::layout::loading::RaceLoading::new(&mut r));
            // step 0 activates the screen and draws the era's mosaic; later steps only move the bar
            // (step 5 also reloads the art with the track's own picture)
            if !rl.active || drawn < 0 {
                rl.step(&mut r, &self.paths, 0, &inputs);
            }
            if step > 0 {
                rl.step(&mut r, &self.paths, step, &inputs);
            }
            let (lw, lh) = self.hud_logical_on(&r);
            let sprites = self.race_loading_sprites(lw, lh);
            let scene = Scene { sprites, clear_color: [0.0, 0.0, 0.0, 1.0], ..Scene::default() };
            if let Err(e) = r.render(&scene, &self.camera) {
                tracing::error!(error = %e, "track loading screen render failed");
            }
            self.renderer = Some(r);
            if let Some(s) = self.fe.web_track_loading.as_mut() {
                s.drawn = step;
            }
            // the shell's panel only goes now that the game's own frame is up (and again if a
            // failed download's Retry put it back)
            crate::web::track_screen(true);
        }
        if let Some(s) = self.fe.web_track_loading.as_mut() {
            if s.ready && !s.resume_requested {
                s.resume_requested = true;
                crate::web::request_race_retry();
            }
        }
    }
}
