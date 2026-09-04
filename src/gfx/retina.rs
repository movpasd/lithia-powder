use glam::{Vec2, vec2};
use sdl3::{
    gpu::{
        BlitInfo, CommandBuffer, Device, Filter, LoadOp, SampleCount, Texture,
        TextureCreateInfo, TextureFormat, TextureType, TextureUsage, Viewport,
    },
    pixels::Color,
};

/// represents the surface which the eyeball sees, and provides utilities for sizing
/// that onto a screen
///
/// the logic herein supports pixel-perfect blitting, preferring to cut pixels off
/// rather than distort.
#[derive(Clone)]
pub struct Retina {
    width: f32,
    height: f32,
    scale: f32,
    /// represents luminance from mesh renderer onto eyeball's retina
    surface: Texture<'static>,
}
impl Retina {
    pub const MAX_WIDTH: u32 = 1920;
    pub const MAX_HEIGHT: u32 = 1080;
    pub const TEXTURE_FORMAT: TextureFormat = TextureFormat::B8g8r8a8Unorm;

    pub fn new(device: &Device, width: f32, height: f32, scale: f32) -> Self {
        let surface = device
            .create_texture(
                TextureCreateInfo::new()
                    .with_type(TextureType::_2D)
                    .with_format(Self::TEXTURE_FORMAT)
                    .with_usage(TextureUsage::COLOR_TARGET | TextureUsage::SAMPLER)
                    // strictly speaking, these should be the max _retina_ sizes, not
                    // _screen_ sizes.
                    .with_width(Self::MAX_WIDTH)
                    .with_height(Self::MAX_HEIGHT)
                    .with_layer_count_or_depth(1)
                    .with_num_levels(1)
                    .with_sample_count(SampleCount::NoMultiSampling),
            )
            .unwrap();
        Self {
            width,
            height,
            scale,
            surface,
        }
    }
    pub fn width(&self) -> f32 {
        self.width
    }
    pub fn height(&self) -> f32 {
        self.height
    }
    pub fn surface(&self) -> Texture<'static> {
        self.surface.clone()
    }

    pub fn prepare(&mut self) {}
    pub fn render(&self, command_buffer: &CommandBuffer, retina_target: RetinaTarget) {
        let ((src_x, src_y, src_w, src_h), (dest_x, dest_y, dest_w, dest_h)) = {
            let ((clip_tl_retpx, clip_diag_retpx), (target_tl_scpx, target_diag_scpx)) =
                self.calc_screen_blit(self.scale, vec2(retina_target.width, retina_target.height));
            (
                (
                    clip_tl_retpx.x as u32,
                    clip_tl_retpx.y as u32,
                    clip_diag_retpx.x as u32,
                    clip_diag_retpx.y as u32,
                ),
                (
                    target_tl_scpx.x as u32,
                    target_tl_scpx.y as u32,
                    target_diag_scpx.x as u32,
                    target_diag_scpx.y as u32,
                ),
            )
        };
        let blit = retina_target
            .preconfigured_blit
            .with_source_texture(&self.surface)
            .with_source_region(0, src_x, src_y, src_w, src_h)
            .with_source_mip(0)
            // .with_destination_texture() --- from pre-configuration
            .with_destination_region(0, dest_x, dest_y, dest_w, dest_h)
            .with_destination_mip(0)
            .with_load_op(LoadOp::CLEAR)
            .with_clear_color(Color::RGB(0, 0, 0))
            .with_filter(Filter::Nearest)
            .with_cycle(false);
        command_buffer.blit_texture(blit);
    }

    pub fn prepare_target<'a>(&self, target: &Texture<'a>) -> RetinaTarget {
        let preconfigured_blit = BlitInfo::default().with_destination_texture(target);
        let (width, height) = (target.width() as f32, target.height() as f32);
        RetinaTarget {
            preconfigured_blit,
            width,
            height,
        }
    }

    /// works out how to blit the image on the retina onto the screen, so the retina
    /// image is scaled and centred on the screen
    ///
    /// returns two rectangles.
    ///
    /// each rectangle is in the format (tl: Vec2, diag: Vec2); `tl` is the top-left
    /// of the rectangle; `diag` is the diagonal from the top-left to the bottom-right,
    /// therefore its two co-ordinates are the rectangle's width and height
    /// respectively. in other words (tl, size) == ([x, y], [w, h]).
    ///
    /// the first returned rectangle is in retpx (retina pixels) and represents the clip
    /// region to cut out of the retina; the second is in scpx (screen pixels) and
    /// represents the blit target region of the cut out region.
    ///
    /// retpx and scpx coordinates are measured in texel space, i.e.: origin in the
    /// top-left corner.
    ///
    /// if `scale` is an integer (i.e.: has fractional part == 0.0) and both sizes in
    /// `screen_size_scpx` are even, all returned co-ordinates should be integers.
    fn calc_screen_blit(
        &self,
        retina_to_screen_scale: f32,
        screen_diag_scpx: Vec2,
    ) -> ((Vec2, Vec2), (Vec2, Vec2)) {
        // 0. useful values
        let screen_centre_scpx = screen_diag_scpx / 2.0;
        let screen_tl_scpx = vec2(0.0, 0.0);
        let screen_br_scpx = screen_tl_scpx + screen_diag_scpx; // br = bottom-right

        let retina_diag_retpx = vec2(self.width, self.height);
        let retina_centre_retpx = retina_diag_retpx / 2.0;

        // 1. calculate scpx coordinates of the blit if there was no clipping (so if the scaled blit is too large )
        let unclipped_diag_scpx = retina_diag_retpx * retina_to_screen_scale;
        let unclipped_tl_scpx = screen_centre_scpx - unclipped_diag_scpx / 2.0;
        let unclipped_br_scpx = screen_centre_scpx + unclipped_diag_scpx / 2.0;

        // 2. constrain the rectangle to the size of the screen to get target
        let target_tl_scpx = unclipped_tl_scpx.clamp(screen_tl_scpx, screen_br_scpx);
        let target_br_scpx = unclipped_br_scpx.clamp(screen_tl_scpx, screen_br_scpx);

        // 3. convert the clip back to retina co-ords to work out clip region
        let target_diag_scpx = target_br_scpx - target_tl_scpx;
        let clip_diag_retpx = target_diag_scpx / retina_to_screen_scale;
        let clip_tl_retpx = retina_centre_retpx - clip_diag_retpx / 2.0;

        (
            (clip_tl_retpx, clip_diag_retpx),
            (target_tl_scpx, target_diag_scpx),
        )
    }

    pub fn viewport(&self) -> Viewport {
        Viewport::new(0.0, 0.0, self.width, self.height, 0.0, 1.0)
    }
}

/// (required to get around the &mut CommandBuffer issue.)
pub struct RetinaTarget {
    preconfigured_blit: BlitInfo,
    width: f32,
    height: f32,
}
