import geoLocationManager from '@ohos.geoLocationManager';

export interface GpsInfo {
  latitude: number;
  longitude: number;
  accuracy: number;
  altitude: number;
  speed: number;
  direction: number;
  timeStamp: number;
}

export class LocationState {
  static async getCurrentLocation(timeoutMs: number = 5000): Promise<GpsInfo | null> {
    try {
      const request: geoLocationManager.CurrentLocationRequest = {
        priority: geoLocationManager.LocationRequestPriority.ACCURACY,
        scenario: geoLocationManager.LocationRequestScenario.DAILY_LIFE_SERVICE,
        timeoutMs,
        maxAccuracy: 50
      };
      const location: geoLocationManager.Location = await geoLocationManager.getCurrentLocation(request);
      const info: GpsInfo = {
        latitude: location.latitude,
        longitude: location.longitude,
        accuracy: location.accuracy,
        altitude: location.altitude,
        speed: location.speed,
        direction: location.direction,
        timeStamp: location.timeStamp
      };
      return info;
    } catch (e) {
      console.warn(`[LocationState] get location failed: ${String(e)}`);
      return null;
    }
  }
}
