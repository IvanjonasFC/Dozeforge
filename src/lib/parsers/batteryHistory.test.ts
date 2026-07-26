import { describe, it, expect } from 'vitest';
import { parseBatteryHistory } from './batteryHistory';

// Real Pixel 8 Pro (Android 14/15) "Format: 2" battery history sample.
const FIXTURE = `Battery History [Format: 2] (102% used, 4208KB used of 4096KB, 151 strings using 7967):
  07-23 12:08:29.838 TIME: 2026-07-23-12-08-29
  07-23 12:08:29.838 093 status=discharging health=good plug=none temp=348 volt=4303 +running +wifi device_idle=full
  07-23 12:08:39.332 093
  07-23 12:08:54.392 093 -running
  07-23 12:09:00.000 092 +screen
  07-23 12:10:03.923 092 +running +wake_lock=1000:"*alarm*:TIME_TICK" wake_reason=0:"479 dhdpcie_host_wake"
  07-23 12:10:33.923 092 -wake_lock
  07-23 12:12:00.000 091 -screen device_idle=off status=charging
`;

describe('parseBatteryHistory (Format 2)', () => {
  it('parses events with real, non-zero timestamps', () => {
    const { events } = parseBatteryHistory(FIXTURE);
    expect(events.length).toBeGreaterThan(3);
    expect(events[events.length - 1]!.t).toBeGreaterThan(0);
  });

  it('extracts battery level and temperature (temp=348 → 34.8 °C)', () => {
    const { events } = parseBatteryHistory(FIXTURE);
    expect(events[0]!.level).toBe(93);
    expect(events[0]!.temp).toBeCloseTo(34.8, 1);
  });

  it('tracks screen / charging / doze transitions', () => {
    const { events } = parseBatteryHistory(FIXTURE);
    expect(events[0]!.doze).toBe(true); // device_idle=full
    const last = events[events.length - 1]!;
    expect(last.doze).toBe(false); // device_idle=off
    expect(last.charging).toBe(true); // status=charging
    expect(last.screen).toBe(false); // -screen
  });

  it('tolerates Windows CRLF line endings (the \\r bug)', () => {
    const { events } = parseBatteryHistory(FIXTURE.replace(/\n/g, '\r\n'));
    expect(events.length).toBeGreaterThan(3);
    expect(events[0]!.level).toBe(93);
  });

  it('attributes wakelock hold time to the holder tag', () => {
    const { holders } = parseBatteryHistory(FIXTURE);
    expect(holders.length).toBeGreaterThan(0);
    expect(holders[0]!.tag).toContain('alarm');
    expect(holders[0]!.ms).toBeGreaterThanOrEqual(30000); // held ~30s
  });

  it('returns empty (no crash) on unrelated input', () => {
    expect(parseBatteryHistory('nothing here').events).toEqual([]);
  });
});
