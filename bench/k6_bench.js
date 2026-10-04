// k6_bench.js - Scénario de benchmark de charge distribué pour Oxid
// Exécution : k6 run -e TARGET_URL=http://192.168.2.142:8080 -e API_KEY=sk_test bench/k6_bench.js

import http from 'k6/http';
import { check, sleep } from 'k6';

const TARGET_URL = __ENV.TARGET_URL || 'http://localhost:8080';
const API_KEY = __ENV.API_KEY || '';

export const options = {
  scenarios: {
    // 1. Warmup & Ramp-up (Omission Coordonnée prévenue par 'ramping-arrival-rate')
    production_load: {
      executor: 'ramping-arrival-rate',
      startRate: 100,
      timeUnit: '1s',
      preAllocatedVUs: 50,
      maxVUs: 500,
      stages: [
        { duration: '30s', target: 500 },  // Phase 1 : Warmup progressif
        { duration: '2m', target: 2000 },  // Phase 2 : Régime nominal
        { duration: '30s', target: 4000 }, // Phase 3 : Pic de charge
        { duration: '30s', target: 100 },  // Phase 4 : Cooldown
      ],
    },
  },
  thresholds: {
    'http_req_duration': ['p(50)<5', 'p(95)<30', 'p(99)<100'], // Objectifs SLA
    'http_req_failed': ['rate<0.001'],                         // 99.9% de succès requis
  },
};

export default function () {
  const params = {
    headers: {
      'Accept': 'application/json, image/jpeg, */*',
    },
  };
  if (API_KEY) {
    params.headers['X-API-Key'] = API_KEY;
  }

  const rand = Math.random();

  if (rand < 0.60) {
    // 60% : Consultation Cache Hit
    const res = http.get(`${TARGET_URL}/api/documents/bench-doc-1/pages/1/render?dpi=120`, params);
    check(res, { 'status is 200': (r) => r.status === 200 });
  } else if (rand < 0.85) {
    // 25% : Gateway Health / Ping
    const res = http.get(`${TARGET_URL}/api/health`, params);
    check(res, { 'status is 200': (r) => r.status === 200 });
  } else if (rand < 0.95) {
    // 10% : Thumbnail
    const res = http.get(`${TARGET_URL}/api/documents/bench-doc-1/pages/1/thumbnail`, params);
    check(res, { 'status is 200': (r) => r.status === 200 });
  } else {
    // 5% : Rendu à froid (DPI dynamique)
    const dpi = Math.floor(Math.random() * 50) + 100;
    const res = http.get(`${TARGET_URL}/api/documents/bench-doc-1/pages/1/render?dpi=${dpi}`, params);
    check(res, { 'status is 200': (r) => r.status === 200 });
  }

  sleep(0.02);
}
