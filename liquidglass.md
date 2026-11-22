# Sonata Liquid Glass Design & Implementation Specification

This specification documents the Liquid Glass rendering pipeline, strict surface-usage rules, shader/blur constants, and Compose integration patterns for **Sonata Android**, derived from:

- BitChord's vendored [`backdrop/`](research/bitchord/app/src/main/java/com/music/bitchord/ui/components/backdrop/) engine (Kyant0/backdrop v2.0.0, Apache-2.0) and integration layer ([`LiquidGlass.kt`](research/bitchord/app/src/main/java/com/music/bitchord/ui/components/LiquidGlass.kt), [`GlassNavBar.kt`](research/bitchord/app/src/main/java/com/music/bitchord/ui/components/GlassNavBar.kt), [`FloatingTabBar.kt`](research/bitchord/app/src/main/java/com/music/bitchord/ui/components/floatingtabbar/FloatingTabBar.kt), [`GlassSelectionPill.kt`](research/bitchord/app/src/main/java/com/music/bitchord/ui/components/floatingtabbar/GlassSelectionPill.kt), [`FrostedTopBar.kt`](research/bitchord/app/src/main/java/com/music/bitchord/ui/components/FrostedTopBar.kt), [`IosMenu.kt`](research/bitchord/app/src/main/java/com/music/bitchord/ui/components/IosMenu.kt)).
- Sonata's [`LiquidGlass.kt`](android/app/src/main/java/dev/sonata/android/LiquidGlass.kt), [`SonataGlassTweaks.kt`](android/app/src/main/java/dev/sonata/android/ui/theme/SonataGlassTweaks.kt), and [`MainActivity.kt`](android/app/src/main/java/dev/sonata/android/MainActivity.kt).
- Visual verification against the 7 BitChord reference screenshots in `/home/mehmetcan/Downloads/`.

---

## 1. Core Liquid Glass Design Rules

Liquid Glass is an optical material whose entire visual value comes from **refracting and blurring live, moving content underneath it**. Applying the shader pipeline over flat, uniform backgrounds wastes GPU fill rate and produces muddied, indistinct rectangles.

### 1.1 Where Liquid Glass MUST Be Used (Live Dynamic Backdrops Only)

Only apply full `Modifier.liquidGlass(...)` where scrollable feeds, artwork, or dynamic surfaces actively pass behind the control:

1. **Floating Bottom Navigation Bar (`GlassNavBar` / `FloatingTabBar`)**:
   - Expanded state: Floating rounded-capsule tab bar + docked full-width Mini Now Playing capsule above it.
   - Collapsed inline state (on scroll down): Active tab pill (left) + compact Now Playing capsule (center) + standalone circular Search pill (right).
   - Both states refract the scrolling Home, Explore, Radio, Library, Search, Detail, and Settings lists underneath (`LocalAppBackdrop`).
2. **Mini Now Playing Accessory (`GlassNowPlaying`)**:
   - Docked above the tab bar (expanded) or nested between the active tab and Search circle (inline), refracting scrolling content below.
3. **Collapsed Library / Radio / Search Floating Chrome**:
   - Floating circular or pill-shaped chrome elements (such as the collapsed Library/Radio bar and floating Search field pill) hovering over scrolling shelves and grids.
4. **Floating Top Controls Over Artwork & Scrolling Feeds**:
   - Circular `<` Back button and grouped `[Share | Avatar]` / `[History | Avatar]` capsule pills floating at the top edge over scrolling pages or full-bleed artist/album hero imagery.

### 1.2 Where Liquid Glass MUST NEVER Be Used (Solid / Flat Surfaces)

Never apply `Modifier.liquidGlass(...)` (nor full backdrop-refraction layers) on opaque, static, or solid-background screens:

1. **Account Sheet / Menu**:
   - Uses a **flat nocturnal Lunaria gradient** (`AccountGradientTop` → `AccountGradientBottom`, see [`LunariaPalette.kt`](android/app/src/main/java/dev/sonata/android/ui/theme/LunariaPalette.kt)) with clean grouped cards and crisp typography. Never wrap the Account sheet background or its internal cards in live lens refraction.
2. **Settings Screen (`CategorizedSettingsSheet`)**:
   - Uses a **flat pure-black / neutral dark gray** hierarchy: `#000000` canvas (`SettingsBg`), `#161618` grouped cards (`SettingsCard`), `#232326` hairline dividers (`SettingsDivider`).
   - Settings rows and group cards are always flat opaque surfaces. Only the floating `<` back button and the bottom `GlassNavBar` hovering *over* the scrolled Settings list use Liquid Glass.
3. **Static Action Buttons Over Uniform Scrims**:
   - Buttons like `Shuffle` / `Favorite` on Album/Artist headers use `Modifier.lightweightLiquidGlass(...)` or `Modifier.liquidGlassRim(...)` (directional specular highlight + `0.5.dp` hairline border over a static translucent fill) without recording or sampling `LayerBackdrop`.
4. **Dense Context Menus & Popovers (`IosMenu` pattern)**:
   - Use **frosted backdrop blur without lens distortion** (`menuBackdropFrost`: `24.dp` blur, `0.25f` scale, `1.8f` saturation, `0.5.dp` hairline rim, `30.dp` continuous corners) rather than the curved refraction lens, keeping dense vertical menu text crisp.

### 1.3 Reference Screenshot Breakdown

Observations from the 7 BitChord reference captures:

| Reference Screenshot | Surface Observed | Material Treatment |
|---|---|---|
| `Screenshot_20261121_201915_BitChord.png` | Account popover over Home feed + expanded `GlassNavBar` + `MiniPlayer` | Popover uses translucent backdrop blur + hairline rim; bottom nav & mini-player refract scrolling tracks behind them. |
| `Screenshot_20261121_202024_BitChord.png` | Collapsed inline `GlassNavBar` + floating avatar on Radio/Mixes | 3-piece collapsed row (`[Radio pill] [Starboy mini-player] [Search circle]`) refracts the colorful mix artwork passing underneath. |
| `Screenshot_20261121_202044_BitChord.png` | Settings (`Ayarlar`) | Flat `#000000` background + flat dark neutral gray grouped cards; floating `<` back button at top-left and `GlassNavBar` at bottom refract scrolled rows. |
| `Screenshot_20261121_203317_BitChord.png` | Artist page (`Kendrick Lamar`) | Circular `<` back and `[Share \| Avatar]` capsule float over hero monochrome photography; header `Shuffle`/`Star` buttons use rim-only glass (`liquidGlassRim`); inline `GlassNavBar` refracts top tracks. |
| `Screenshot_20261121_203323_BitChord.png` | Library (`Kitaplık`) scrolled | Floating `[History \| Avatar]` capsule at top-right; collapsed inline `GlassNavBar` refracting artist grid cards. |
| `Screenshot_20261121_203355_BitChord.png` | Track context menu over Now Playing | `24.dp` frost (`0.25f` scale) without lens warping so 13+ stacked action rows stay legible over dark artwork. |
| `Screenshot_20261121_203633_BitChord.png` | Tablet / landscape Home + update modal + volume HUD | Floating top-right `[Share \| Avatar]` pill, centered `GlassNavBar` + `MiniPlayer` refracting the 3-column grid, vertical right-edge volume HUD pill. |

---

## 2. Reusable Constants & Material Parameters

### 2.1 Platform & Capability Gates

- **Minimum Blur Support (`isGlassSupported`)**: Android 12 (`Build.VERSION_CODES.S`, API 31) for `android.graphics.RenderEffect` on `RenderNode`.
- **Lens Refraction Shader Support (`isRuntimeShaderSupported`)**: Android 13 (`Build.VERSION_CODES.TIRAMISU`, API 33) for AGSL `RuntimeShader` (`RenderEffect.createRuntimeShaderEffect`).
- **Fallback below API 31**: `lightweightLiquidGlass` renders a static translucent tint + `0.5.dp` hairline border without backdrop capture.

### 2.2 Surface, Blur, and Lens Constants

| Constant | Value | Purpose |
|---|---|---|
| `VIBRANCY` | `1f` | Feeds `colorControls(saturation = 1f + 0.5f * VIBRANCY)` = **`1.5f` saturation boost** so refracted artwork colors stay vivid through glass. |
| `BLUR_RADIUS_DP` | `8f` (`8.dp`) | Standard Gaussian blur radius before `GLASS_RESOLUTION_SCALE`. |
| `LENS_HEIGHT` | `0.5f` | Normalized edge band thickness (`LENS_HEIGHT * LENS_MAX_DP` = **`24.dp`**). |
| `LENS_AMOUNT` | `0.5f` | Normalized refraction displacement (`LENS_AMOUNT * LENS_MAX_DP` = **`24.dp`**). |
| `LENS_MAX_DP` | `48f` (`48.dp`) | Maximum physical lens extent. |
| `SURFACE_OPACITY` | `0.4f` (40%) | Surface tint alpha over refracted backdrop (`#FAFAFA` light / `#121212` dark; Sonata card variant uses `0.44f` dark / `0.52f` light). |
| `GLASS_RESOLUTION_SCALE` | `0.33f` | Backdrop recorded at 1/3 resolution (~11% pixel count, 9x fewer shader samples). |
| `FROST_BLUR_RADIUS_DP` | `16f` (`16.dp`) | Wider blur used when `glassPerformance` disables the AGSL lens shader. |
| `FROST_RESOLUTION_SCALE` | `0.25f` | Reduced recording scale (~6% pixels) under `glassPerformance`. |
| `GLASS_EDGE_WIDTH` | `0.5.dp` | Hairline border width on non-shader fallbacks and top-bar pills. |
| `GLASS_EDGE_COLOR` | `Color.White.copy(alpha = 0.10f)` | Subtle specular edge hairline. |
| `DARK_INDICATOR_OPACITY` | `0.14f` | Resting selected-tab indicator fill in dark theme (`Color.White @ 0.50f` in light theme). |
| `SQUIRCLE_PILL_EXPONENT` | `2.1f` | Superellipse identifier; `SquirclePillShape` returns `Outline.Rounded` for the HWUI fast path. |

### 2.3 Highlight & Shadow Constants

From `backdrop/highlight/Highlight.kt`, `HighlightStyle.kt`, and `shadow/Shadow.kt`:

- **`Highlight.Default`**:
  - `width = 0.5.dp`, `blurRadius = 0.25.dp`, `alpha = 1f`
  - `HighlightStyle.Default`: `color = Color.White.copy(alpha = 0.50f)`, `blendMode = BlendMode.Plus`, `angle = 45f` (top-left light source), `falloff = 1f`.
- **`Shadow.Default`**:
  - `radius = 24.dp`, `offset = DpOffset(0.dp, 4.dp)`, `color = Color.Black.copy(alpha = 0.10f)`.
  - **Important**: Because `drawBackdrop` already renders `Shadow.Default`, set `FloatingTabBarDefaults.elevations(inlineElevation = 0.dp, expandedElevation = 0.dp)` to avoid paying twice for an offscreen shadow layer.

### 2.4 Traveling Selection Pill Physics (`GlassSelectionPill` / `GlassPillMotion`)

When the user taps or drags across tabs in `FloatingTabBar`, the resting flat fill lifts into a refracting lens pill above the bar:

- **Lifted Lens Geometry**: `PILL_GROW_HEIGHT = 9.dp`, `LENS_HEIGHT = 12.dp`, `LENS_AMOUNT = 14.dp`, `PILL_SHADOW = Shadow(radius = 12.dp, offset = (0, 2).dp, color = Black @ 0.30f)`.
- **Spring Constants**:
  - Travel: `TRAVEL_STIFFNESS = 280.0`, `TRAVEL_DAMPING = 31.4`
  - Lift X/Y: `LIFT_STIFFNESS = 250.0`, `LIFT_DAMPING_X = 19.0`, `LIFT_DAMPING_Y = 22.1` (width overshoots slightly more than height)
  - Material fade: `MATERIAL_STIFFNESS = 1000.0`, `MATERIAL_DAMPING = 63.3`
  - Handover & Squash: `HANDOVER_START = 0.92`, `HANDOVER_TAU = 0.09`, `GLASS_RETURN_TAU = 0.05`, `FOLLOW_TAU = 0.05`, `SIGN_TAU = 0.25`, `MAX_DEFORMATION = 0.12` (±12%).
  - `GlassSpring` (for tab bar transitions): `spring<Float>(dampingRatio = 0.72f, stiffness = 320f)`.

---

## 3. RenderEffect & AGSL Shader Pipeline

Every `Modifier.liquidGlass(...)` surface executes the following order of operations inside `DrawBackdropModifier`:

1. **Source Recording (`LayerBackdrop`)**:
   - The scrollable page container applies `Modifier.layerBackdrop(appBackdrop)`.
   - **Mandatory opaque floor**: `rememberLayerBackdrop(onDraw = { drawRect(windowBackground); drawContent() })`. Without `drawRect(windowBackground)`, unpainted gaps between list items remain transparent in the recorded `GraphicsLayer`, causing unblurred page text to bleed sharply through the glass.
   - When `LocalReduceDynamicBlur.current` is `true`, skip `Modifier.layerBackdrop(appBackdrop)` so the extra draw pass is eliminated completely.
2. **Downsampled Effect Pass (`backdropScale`)**:
   - Records the overlapping region into an offscreen `GraphicsLayer` scaled by `GLASS_RESOLUTION_SCALE` (`0.33f`) or `FROST_RESOLUTION_SCALE` (`0.25f`).
   - Pixel-unit radii (`blurPx`, `lensHeightPx`, `lensAmountPx`) are pre-multiplied by `resolutionScale`.
3. **Chained `RenderEffect` Graph**:
   - Step A — **Vibrancy**: `colorControls(saturation = 1.5f)` (`ColorFilterEffect` color matrix).
   - Step B — **Gaussian Blur**: `BlurEffect(blurPx, blurPx, TileMode.Clamp)` (API 31+).
   - Step C — **AGSL SDF Lens Refraction** (API 33+, skipped when `glassPerformance = true`):
     - `RoundedRectRefractionShaderString` computes the signed distance `sd = sdRoundedRect(centeredCoord, halfSize, radius)`.
     - Inside the border band (`-sd < refractionHeight`), displacement follows `d = (1.0 - sqrt(1.0 - x * x)) * (-refractionAmount)` where `x = 1.0 - (-sd / refractionHeight)`.
     - Refracted UV offset travels along `normalize(gradSdRoundedRect(...) + depthEffect * normalize(centeredCoord))`.
     - Optional `chromaticAberration` splits into 7 spectral taps (`RoundedRectRefractionWithDispersionShaderString`); Sonata keeps `chromaticAberration = false` on static bars for crispness and reserves dispersion for dynamic lenses if desired.
4. **Surface Tint (`onDrawSurface`)**:
   - Draws `surfaceTintColor` (`#121212 @ 0.40f` dark / `#FAFAFA @ 0.40f` light) over the refracted backdrop.
5. **Directional Rim (`HighlightElement`) & Drop Shadow (`ShadowElement`)**:
   - `DefaultHighlightShaderString` evaluates `pow(abs(dot(grad, vec2(cos(45°), sin(45°)))), falloff)` along the rounded-rect outline and composites with `BlendMode.Plus`.

---

## 4. Kotlin / Compose Reference Snippets

### 4.1 Recording the Opaque Page Backdrop

```kotlin
val windowBackground = if (settings.themeMode == ThemeMode.AMOLED) {
    Color.Black
} else {
    MaterialTheme.colorScheme.background
}
val paintBackdrop: ContentDrawScope.() -> Unit = remember(windowBackground) {
    {
        drawRect(windowBackground) // Prevents sharp text bleed-through on transparent gaps
        drawContent()
    }
}
val appBackdrop = rememberLayerBackdrop(onDraw = paintBackdrop)

// On the scrollable content root (skipped when Reduce Dynamic Blur is enabled):
val recordLayer = glassEnabled && !reduceDynamicBlur
Box(
    modifier = Modifier
        .fillMaxSize()
        .then(if (recordLayer) Modifier.layerBackdrop(appBackdrop) else Modifier)
        .nestedScroll(navBarScroll),
) {
    PageContent(...)
}
```

### 4.2 Primary `Modifier.liquidGlass` & Fallback Modifiers

```kotlin
// Use ONLY over real scrollable/dynamic content (bottom nav, mini-player, floating top controls).
@Composable
fun Modifier.adaptiveLiquidGlass(
    shape: CornerBasedShape,
    fallbackColor: Color = MaterialTheme.colorScheme.surface.copy(alpha = 0.92f),
    surfaceTint: Color? = null,
): Modifier = if (LocalLiquidGlassEnabled.current && isGlassSupported()) {
    this.liquidGlass(shape = shape, surfaceTint = surfaceTint)
} else {
    this.lightweightLiquidGlass(shape = shape, fallbackColor = fallbackColor)
}

// Use on static buttons over stable scrims (no backdrop capture overhead):
ButtonModifier
    .background(Color.White.copy(alpha = 0.14f), CircleShape)
    .liquidGlassRim(CircleShape)
```

### 4.3 Multi-Surface Factory Pattern (Performance Rule)

Every `Modifier.liquidGlass` instance caches its shape outline keyed on the last drawn component size. **Never pass a single `val mod = Modifier.liquidGlass(pillShape)` to multiple surfaces of different dimensions** (e.g., tab pill + mini-player + circular Search button):

```kotlin
val pillShape = remember { SquirclePillShape() } // HWUI Outline.Rounded fast path
val glassSurfaceFactory: @Composable () -> Modifier = {
    Modifier.liquidGlass(shape = pillShape)
}
```

### 4.4 Hardware-Fast Capsule Shape

```kotlin
class SquirclePillShape(val exponent: Float = 2.1f) :
    CornerBasedShape(CornerSize(50), CornerSize(50), CornerSize(50), CornerSize(50)) {
    override fun copy(
        topStart: CornerSize,
        topEnd: CornerSize,
        bottomEnd: CornerSize,
        bottomStart: CornerSize,
    ): CornerBasedShape = SquirclePillShape(exponent)

    override fun createOutline(
        size: Size,
        topStart: Float,
        topEnd: Float,
        bottomEnd: Float,
        bottomStart: Float,
        layoutDirection: LayoutDirection,
    ): Outline {
        val r = size.minDimension / 2f
        if (r <= 0f) return Outline.Rectangle(size.toRect())
        return Outline.Rounded(RoundRect(size.toRect(), CornerRadius(r, r)))
    }
}
```
