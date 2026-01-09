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

  static async getAddressFromLocation(
    latitude: number,
    longitude: number,
    locale: string = 'zh-CN'
  ): Promise<string> {
    try {
      const request: geoLocationManager.ReverseGeoCodeRequest = {
        latitude,
        longitude,
        locale,
        maxItems: 1
      };
      const addresses: geoLocationManager.GeoAddress[] =
        await geoLocationManager.getAddressesFromLocation(request);
      if (addresses.length === 0) {
        return '';
      }
      return LocationState.formatGeoAddress(addresses[0]);
    } catch (e) {
      console.warn(`[LocationState] reverse geocode failed: ${String(e)}`);
      return '';
    }
  }

  private static formatGeoAddress(address: geoLocationManager.GeoAddress): string {
    const parts: string[] = [];
    const candidates: Array<string | undefined> = [
      address.countryName,
      address.administrativeArea,
      address.subAdministrativeArea,
      address.locality,
      address.subLocality,
      address.roadName,
      address.subRoadName,
      address.premises,
      address.placeName
    ];
    for (let i = 0; i < candidates.length; i++) {
      const value = candidates[i];
      if (value && value.length > 0) {
        parts.push(value);
      }
    }
    return parts.join('');
  }
}
