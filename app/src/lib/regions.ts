/** Places for exit nodes: Tailscale's DERP regions, which say where a node's nearest relay is, and a few more cities
 *  where VPSes are rented. Codes are the region's own where it has one. */
export interface Region {
  code: string;
  city: string;
  country: string;
  lat: number;
  lon: number;
}

export const REGIONS: Region[] = [
  { code: "ams", city: "Amsterdam", country: "Netherlands", lat: 52.37, lon: 4.9 },
  { code: "fra", city: "Frankfurt", country: "Germany", lat: 50.11, lon: 8.68 },
  { code: "nue", city: "Nuremberg", country: "Germany", lat: 49.45, lon: 11.08 },
  { code: "lhr", city: "London", country: "United Kingdom", lat: 51.51, lon: -0.13 },
  { code: "par", city: "Paris", country: "France", lat: 48.86, lon: 2.35 },
  { code: "mad", city: "Madrid", country: "Spain", lat: 40.42, lon: -3.7 },
  { code: "waw", city: "Warsaw", country: "Poland", lat: 52.23, lon: 21.01 },
  { code: "hel", city: "Helsinki", country: "Finland", lat: 60.17, lon: 24.94 },
  { code: "sto", city: "Stockholm", country: "Sweden", lat: 59.33, lon: 18.07 },
  { code: "vie", city: "Vienna", country: "Austria", lat: 48.21, lon: 16.37 },
  { code: "zrh", city: "Zurich", country: "Switzerland", lat: 47.37, lon: 8.54 },
  { code: "ist", city: "Istanbul", country: "Turkey", lat: 41.01, lon: 28.98 },
  { code: "dbi", city: "Dubai", country: "United Arab Emirates", lat: 25.2, lon: 55.27 },
  { code: "jnb", city: "Johannesburg", country: "South Africa", lat: -26.2, lon: 28.05 },
  { code: "nai", city: "Nairobi", country: "Kenya", lat: -1.29, lon: 36.82 },
  { code: "blr", city: "Bangalore", country: "India", lat: 12.97, lon: 77.59 },
  { code: "sin", city: "Singapore", country: "Singapore", lat: 1.35, lon: 103.82 },
  { code: "hkg", city: "Hong Kong", country: "Hong Kong", lat: 22.32, lon: 114.17 },
  { code: "tok", city: "Tokyo", country: "Japan", lat: 35.68, lon: 139.69 },
  { code: "syd", city: "Sydney", country: "Australia", lat: -33.87, lon: 151.21 },
  { code: "hnl", city: "Honolulu", country: "United States", lat: 21.31, lon: -157.86 },
  { code: "sea", city: "Seattle", country: "United States", lat: 47.61, lon: -122.33 },
  { code: "sfo", city: "San Francisco", country: "United States", lat: 37.77, lon: -122.42 },
  { code: "lax", city: "Los Angeles", country: "United States", lat: 34.05, lon: -118.24 },
  { code: "den", city: "Denver", country: "United States", lat: 39.74, lon: -104.99 },
  { code: "dfw", city: "Dallas", country: "United States", lat: 32.78, lon: -96.8 },
  { code: "ord", city: "Chicago", country: "United States", lat: 41.88, lon: -87.63 },
  { code: "tor", city: "Toronto", country: "Canada", lat: 43.65, lon: -79.38 },
  { code: "nyc", city: "New York", country: "United States", lat: 40.71, lon: -74.01 },
  { code: "iad", city: "Ashburn", country: "United States", lat: 39.04, lon: -77.49 },
  { code: "mia", city: "Miami", country: "United States", lat: 25.76, lon: -80.19 },
  { code: "sao", city: "São Paulo", country: "Brazil", lat: -23.55, lon: -46.63 },
];

export const regionOf = (code: string | undefined | null) => REGIONS.find((r) => r.code === code);
