import wallpaper from '@ohos.wallpaper';
import image from '@ohos.multimedia.image';

export class WallpaperState {
  static async getCurrentWallpaper(): Promise<image.PixelMap | null> {
    try {
      const pixelMap = await wallpaper.getWallpaperByState(
        wallpaper.WallpaperType.WALLPAPER_SYSTEM,
        wallpaper.FoldState.NORMAL,
        wallpaper.RotateState.PORTRAIT
      );
      return pixelMap;
    } catch (e) {
      console.warn(`[WallpaperState] get wallpaper failed: ${String(e)}`);
      return null;
    }
  }
}
