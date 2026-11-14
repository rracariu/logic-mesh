import type { EngineCommand } from 'logic-mesh';
import { z } from 'zod';
import {
  connectorRegistered,
  defineJsBlocks,
  unregisterConnector,
} from 'logic-mesh';

// Plain async JS functions exposed as blocks for the "AI Weather
// Advisor" example. Each one is a `Request` block whose `connector` pin
// is 'js' and whose `address` pin names the function: the block's `in`
// value is the argument, the resolved value lands on `out`, and a
// thrown error faults the block with its message.
//
// `geocode` and `weather` call the free Open-Meteo APIs (no key);
// `advise` asks a local Ollama model for structured advice. Ollama
// accepts requests from localhost origins by default, so the dev
// server's page calls it directly. VITE_OLLAMA_URL and
// VITE_OLLAMA_MODEL override the defaults below.

export const JS_CONNECTOR_NAME = 'js';

const OLLAMA_URL = import.meta.env.VITE_OLLAMA_URL ?? 'http://localhost:11434';
const OLLAMA_MODEL = import.meta.env.VITE_OLLAMA_MODEL ?? 'mimo-v2.6';

const Advice = z.object({
  summary: z
    .string()
    .describe(
      'One or two friendly sentences on how to dress and what to bring. At most 160 characters.',
    ),
  umbrella: z.boolean().describe('Bring an umbrella?'),
  jacket: z.boolean().describe('Wear a jacket?'),
  sunscreen: z.boolean().describe('Wear sunscreen?'),
});

const ADVICE_FORMAT = z.toJSONSchema(Advice);

const SYSTEM = `You advise people heading outside, based on the current weather in their city.
You receive the conditions as JSON: temperature and feels-like in °C, relative humidity in %,
precipitation in mm over the last hour, wind in km/h, a short description, and whether it is daytime.
Base every field on those numbers. Sunscreen only makes sense in daytime.
The summary is advice, not a weather report: say what to wear and bring, without repeating the numbers.
Reply with JSON only, matching the requested schema.`;

interface Place {
  name: string;
  country: string;
  lat: number;
  lon: number;
}

async function getJson(url: string, init?: RequestInit): Promise<unknown> {
  const response = await fetch(url, init);
  const body: unknown = await response.json().catch(() => null);
  if (!response.ok) {
    const detail =
      body && typeof body === 'object' && 'error' in body
        ? String(body.error)
        : response.statusText;
    throw new Error(`${response.status} ${detail}`);
  }
  return body;
}

// WMO weather interpretation codes, as Open-Meteo reports them.
function describe(code: number): string {
  if (code === 0) return 'clear sky';
  if (code <= 2) return 'partly cloudy';
  if (code === 3) return 'overcast';
  if (code <= 48) return 'fog';
  if (code <= 57) return 'drizzle';
  if (code <= 67) return 'rain';
  if (code <= 77) return 'snow';
  if (code <= 82) return 'rain showers';
  if (code <= 86) return 'snow showers';
  return 'thunderstorm';
}

// Created on first attach, in dev builds only, so production bundles
// leave the demo out.
const createServices = () =>
  defineJsBlocks(
    {
      /** City name → `{ name, country, lat, lon }`. */
      geocode: async (city): Promise<Place> => {
        const url = new URL('https://geocoding-api.open-meteo.com/v1/search');
        url.searchParams.set('name', String(city));
        url.searchParams.set('count', '1');
        url.searchParams.set('language', 'en');
        const body = (await getJson(url.href)) as {
          results?: {
            name: string;
            country: string;
            latitude: number;
            longitude: number;
          }[];
        };
        const hit = body.results?.[0];
        if (!hit) throw new Error(`no place named '${String(city)}'`);
        return {
          name: hit.name,
          country: hit.country,
          lat: hit.latitude,
          lon: hit.longitude,
        };
      },

      /** `{ lat, lon, name }` → the current conditions there. */
      weather: async (place) => {
        const { lat, lon, name } = place as Place;
        const url = new URL('https://api.open-meteo.com/v1/forecast');
        url.searchParams.set('latitude', String(lat));
        url.searchParams.set('longitude', String(lon));
        url.searchParams.set(
          'current',
          'temperature_2m,apparent_temperature,relative_humidity_2m,precipitation,weather_code,wind_speed_10m,is_day',
        );
        url.searchParams.set('timezone', 'auto');
        const { current: c } = (await getJson(url.href)) as {
          current: Record<string, number>;
        };
        return {
          city: name,
          temp: c.temperature_2m,
          feelsLike: c.apparent_temperature,
          humidity: c.relative_humidity_2m,
          precipitation: c.precipitation,
          wind: c.wind_speed_10m,
          conditions: describe(c.weather_code),
          isDay: c.is_day === 1,
        };
      },

      /** Current conditions → `{ summary, umbrella, jacket, sunscreen }` from the local model. */
      advise: async (weather) => {
        const body = (await getJson(`${OLLAMA_URL}/api/chat`, {
          method: 'POST',
          headers: { 'content-type': 'application/json' },
          body: JSON.stringify({
            model: OLLAMA_MODEL,
            stream: false,
            format: ADVICE_FORMAT,
            options: { temperature: 0.2 },
            messages: [
              { role: 'system', content: SYSTEM },
              { role: 'user', content: JSON.stringify(weather) },
            ],
          }),
        })) as { message?: { content?: string } };
        const reply = body.message?.content ?? '';
        let parsed: unknown;
        try {
          parsed = JSON.parse(reply);
        } catch {
          throw new Error(`${OLLAMA_MODEL} did not reply with JSON`);
        }
        const advice = Advice.safeParse(parsed);
        if (!advice.success) {
          throw new Error(
            `${OLLAMA_MODEL} replied off-schema: ${advice.error.message}`,
          );
        }
        return advice.data;
      },
    },
    { name: JS_CONNECTOR_NAME },
  );

let services: ReturnType<typeof createServices> | undefined;

/**
 * Attaches the demo functions to the running engine. Call it where the
 * UI connector is attached: at start and after every reset. A Vite HMR
 * re-init leaves the previous module's connector registered under the
 * same name, which `attach` refuses; that stale entry is dropped and
 * the attach retried. A no-op outside the dev server, like the example
 * that uses these functions.
 */
export async function attachDemoServices(
  command: EngineCommand,
): Promise<void> {
  if (!import.meta.env.DEV) return;
  services ??= createServices();
  try {
    await services.attach(command);
  } catch (err) {
    if (!connectorRegistered(JS_CONNECTOR_NAME)) throw err;
    if ((await command.listConnectors()).includes(JS_CONNECTOR_NAME)) {
      await command.removeConnector(JS_CONNECTOR_NAME);
    } else {
      unregisterConnector(JS_CONNECTOR_NAME);
    }
    await services.attach(command);
  }
}
