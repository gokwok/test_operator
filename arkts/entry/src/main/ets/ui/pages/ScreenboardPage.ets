import image from '@ohos.multimedia.image';
import { WallpaperState } from '../../features/agent_controller/state_collector/WallpaperState';

@Entry
@Component
struct ScreenboardPage {
  @State private wallpaper: image.PixelMap | null = null;

  aboutToAppear(): void {
    void this.loadWallpaper();
  }

  build() {
    Stack() {
      if (this.wallpaper) {
        Image(this.wallpaper)
          .width('100%')
          .height('100%')
          .objectFit(ImageFit.Cover)
      }
    }
    .width('100%')
    .height('100%')
    .backgroundColor(Color.Black)
  }

  private async loadWallpaper(): Promise<void> {
    this.wallpaper = await WallpaperState.getCurrentWallpaper();
  }
}
