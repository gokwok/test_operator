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
  private static readonly cacheTtlMs: number = 30000;
  private static readonly fastAccuracyThreshold: number = 30;
  private static cachedLocation: GpsInfo | null = null;
  private static cachedAtMs: number = 0;

  static async getCurrentLocation(timeoutMs: number = 5000): Promise<GpsInfo | null> {
    const cached = LocationState.readCache();
    if (cached) {
      return cached;
    }
    const lastLocation = LocationState.getLastLocation();
    if (lastLocation && lastLocation.accuracy <= LocationState.fastAccuracyThreshold) {
      LocationState.writeCache(lastLocation);
      void LocationState.refreshHighAccuracy(timeoutMs);
      return lastLocation;
    }
    try {
      const info = await LocationState.requestLocation(timeoutMs, true);
      if (info) {
        LocationState.writeCache(info);
        return info;
      }
    } catch (e) {
      console.warn(`[LocationState] get location failed: ${String(e)}`);
    }
    if (lastLocation) {
      LocationState.writeCache(lastLocation);
      return lastLocation;
    }
    return null;
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

  private static async refreshHighAccuracy(timeoutMs: number): Promise<void> {
    const info = await LocationState.requestLocation(timeoutMs, true);
    if (info) {
      LocationState.writeCache(info);
    }
  }

  private static requestLocation(timeoutMs: number, highAccuracy: boolean): Promise<GpsInfo | null> {
    const request: geoLocationManager.CurrentLocationRequest = {
      priority: highAccuracy ? geoLocationManager.LocationRequestPriority.ACCURACY :
        geoLocationManager.LocationRequestPriority.FIRST_FIX,
      scenario: highAccuracy ? geoLocationManager.LocationRequestScenario.NAVIGATION :
        geoLocationManager.LocationRequestScenario.DAILY_LIFE_SERVICE,
      timeoutMs,
      maxAccuracy: highAccuracy ? 5 : 30
    };
    return geoLocationManager.getCurrentLocation(request)
      .then((location: geoLocationManager.Location) => {
        return LocationState.mapLocation(location);
      })
      .catch((_e) => {
        return null;
      });
  }

  private static getLastLocation(): GpsInfo | null {
    try {
      const location: geoLocationManager.Location = geoLocationManager.getLastLocation();
      return LocationState.mapLocation(location);
    } catch (_e) {
      return null;
    }
  }

  private static mapLocation(location: geoLocationManager.Location): GpsInfo {
    return {
      latitude: location.latitude,
      longitude: location.longitude,
      accuracy: location.accuracy,
      altitude: location.altitude,
      speed: location.speed,
      direction: location.direction,
      timeStamp: location.timeStamp
    };
  }

  private static readCache(): GpsInfo | null {
    if (!LocationState.cachedLocation) {
      return null;
    }
    const age = Date.now() - LocationState.cachedAtMs;
    if (age > LocationState.cacheTtlMs) {
      LocationState.cachedLocation = null;
      LocationState.cachedAtMs = 0;
      return null;
    }
    return LocationState.cachedLocation;
  }

  private static writeCache(location: GpsInfo): void {
    LocationState.cachedLocation = location;
    LocationState.cachedAtMs = Date.now();
  }
}
