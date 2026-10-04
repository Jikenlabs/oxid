#!/usr/bin/env python3
"""
Oxid Benchmark & Performance Stress-Test Suite
Conforme au Protocole Officiel de Benchmark Oxid (docs/PROTOCOLE_BENCHMARK.md).
Mesure sans dépendance externe : Débit (req/s), Bande passante (Mo/s),
et distribution fine des latences (Min, p50, p75, p90, p95, p99, p99.9, Max).
Fonctionne en local ou à distance depuis un autre poste.
"""

import argparse
import base64
import http.client
import io
import math
import os
import random
import sys
import threading
import time
import urllib.parse
from concurrent.futures import ThreadPoolExecutor, as_completed

# Couleurs ANSI
GREEN = "\033[92m"
YELLOW = "\033[93m"
RED = "\033[91m"
CYAN = "\033[96m"
BOLD = "\033[1m"
RESET = "\033[0m"

# Document PDF minimal valide (2 pages) encodé en base64 pour autonomie complète
SAMPLE_PDF_B64 = (
    "JVBERi0xLjQKMSAwIG9iago8PC9UeXBlL1BhZ2VzL0tpZHNbNSAwIFIgNyAwIFJdL0NvdW50IDI+Pg"
    "plbmRvYmoKMiAwIG9iago8PC9UeXBlL0ZvbnQvU3VidHlwZS9UeXBlMS9CYXNlRm9udC9IZWx2ZXRp"
    "Y2E+PgplbmRvYmoKMyAwIG9iago8PC9Gb250PDwvRjEgMiAwIFI+Pj4+CmVuZG9iago0IDAgb2JqCj"
    "w8L0xlbmd0aCA2OT4+c3RyZWFtCkJUCi9GMSAxNCBUZgoxIDAgMCAxIDUwIDc1MCBUbQooU2FtcGxl"
    "IFRlc3QgRG9jdW1lbnQgLSBQYWdlIDEpIFRqCkVUCgplbmRzdHJlYW0gCmVuZG9iago1IDAgb2JqCj"
    "w8L1R5cGUvUGFnZS9QYXJlbnQgMSAwIFIvTWVkaWFCb3hbMCAwIDU5NSA4NDJdL1Jlc291cmNlcyAz"
    "IDAgUi9Db250ZW50cyA0IDAgUj4+CmVuZG9iago2IDAgb2JqCjw8L0xlbmd0aCA2OT4+c3RyZWFtCk"
    "JUCi9GMSAxNCBUZgoxIDAgMCAxIDUwIDc1MCBUbQooU2FtcGxlIFRlc3QgRG9jdW1lbnQgLSBQYWdl"
    "IDIpIFRqCkVUCgplbmRzdHJlYW0gCmVuZG9iago3IDAgb2JqCjw8L1R5cGUvUGFnZS9QYXJlbnQgMS"
    "AwIFIvTWVkaWFCb3hbMCAwIDU5NSA4NDJdL1Jlc291cmNlcyAzIDAgUi9Db250ZW50cyA2IDAgUj4+"
    "CmVuZG9iago4IDAgb2JqCjw8L1R5cGUvQ2F0YWxvZy9QYWdlcyAxIDAgUj4+CmVuZG9iago5IDAgb2"
    "JqCjw8L1Jvb3QgOCAwIFIvVHlwZS9YUmVmL1NpemUgMTAvV1sxIDQgMl0vSW5kZXhbMSA5XS9MZW5n"
    "dGggNjM+PnN0cmVhbQoBAAAACQAAAQAAAEIAAAEAAACBAAABAAAApwAAAQAAARwAAAEAAAF8AAABAA"
    "AB8QAAAQAAAlEAAAEAAAJ+AAAKZW5kc3RyZWFtIAplbmRvYmoKCnN0YXJ0eHJlZgo2MzgKJSVFT0Y="
)


class BenchmarkResult:
    def __init__(self, name: str):
        self.name = name
        self.latencies_ms = []
        self.bytes_transferred = 0
        self.success_count = 0
        self.error_count = 0
        self.duration_secs = 0.0

    @property
    def total_requests(self):
        return self.success_count + self.error_count

    @property
    def rps(self):
        return self.total_requests / self.duration_secs if self.duration_secs > 0 else 0.0

    @property
    def mbps(self):
        return (self.bytes_transferred / (1024 * 1024)) / self.duration_secs if self.duration_secs > 0 else 0.0

    def percentile(self, p: float):
        if not self.latencies_ms:
            return 0.0
        k = (len(self.latencies_ms) - 1) * (p / 100.0)
        f = math.floor(k)
        c = math.ceil(k)
        if f == c:
            return self.latencies_ms[int(k)]
        return self.latencies_ms[int(f)] * (c - k) + self.latencies_ms[int(c)] * (k - f)

    def print_summary(self):
        self.latencies_ms.sort()
        avg_lat = sum(self.latencies_ms) / len(self.latencies_ms) if self.latencies_ms else 0.0
        p50 = self.percentile(50)
        p75 = self.percentile(75)
        p90 = self.percentile(90)
        p95 = self.percentile(95)
        p99 = self.percentile(99)
        p999 = self.percentile(99.9)
        max_lat = self.latencies_ms[-1] if self.latencies_ms else 0.0
        min_lat = self.latencies_ms[0] if self.latencies_ms else 0.0

        p99_ratio = (p99 / p50) if p50 > 0 else 1.0

        print(f"\n{BOLD}{CYAN}=== RÉSULTATS : {self.name} ==={RESET}")
        print(f"  * Durée effective        : {self.duration_secs:.2f} s")
        print(f"  * Requêtes servies       : {BOLD}{GREEN}{self.total_requests:,}{RESET} ({self.success_count:,} succès, {self.error_count} erreurs)")
        print(f"  * Débit moyen            : {BOLD}{GREEN}{self.rps:,.2f} req/s{RESET}")
        print(f"  * Bande passante         : {self.mbps:.2f} Mo/s ({self.bytes_transferred / (1024*1024):.2f} Mo transférés)")
        print(f"\n  {BOLD}Distribution des latences (ms) :{RESET}")
        print(f"    Min   : {min_lat:8.2f} ms")
        print(f"    Moy   : {avg_lat:8.2f} ms")
        print(f"    p50   : {BOLD}{GREEN}{p50:8.2f} ms{RESET}")
        print(f"    p75   : {p75:8.2f} ms")
        print(f"    p90   : {p90:8.2f} ms")
        print(f"    p95   : {p95:8.2f} ms")
        print(f"    p99   : {BOLD}{YELLOW if p99_ratio < 10 else RED}{p99:8.2f} ms{RESET}")
        print(f"    p99.9 : {p999:8.2f} ms")
        print(f"    Max   : {max_lat:8.2f} ms")
        print(f"    Ratio p99/p50 : {p99_ratio:.1f}x " + (f"{RED}(Contention / Queueing détecté!){RESET}" if p99_ratio > 15 else f"{GREEN}(Stabilité satisfaisante){RESET}"))


class OxidBenchClient:
    def __init__(self, target_url: str, api_key: str = "", verbose: bool = False, doc_id: str = ""):
        self.parsed = urllib.parse.urlparse(target_url)
        self.is_ssl = self.parsed.scheme == "https"
        self.host = self.parsed.hostname or "localhost"
        self.port = self.parsed.port or (443 if self.is_ssl else 80)
        self.target_url = target_url.rstrip("/")
        self.api_key = api_key
        self.verbose = verbose
        self.sample_doc_id = doc_id

    def get_connection(self) -> http.client.HTTPConnection:
        if self.is_ssl:
            import ssl
            ctx = ssl.create_default_context()
            ctx.check_hostname = False
            ctx.verify_mode = ssl.CERT_NONE
            return http.client.HTTPSConnection(self.host, self.port, timeout=10, context=ctx)
        return http.client.HTTPConnection(self.host, self.port, timeout=10)

    def check_health(self) -> bool:
        try:
            conn = self.get_connection()
            headers = {}
            if self.api_key:
                headers["X-API-Key"] = self.api_key
            conn.request("GET", "/api/health", headers=headers)
            res = conn.getresponse()
            body = res.read().decode("utf-8", errors="ignore")
            conn.close()
            if self.verbose:
                print(f"[HEALTH] {body}")
            return res.status == 200
        except Exception as e:
            print(f"{RED}Échec de connexion vers {self.target_url} : {e}{RESET}")
            return False

    def ensure_sample_document(self) -> str:
        """S'assure qu'un document existe pour les tests de pages et thumbnails."""
        if self.sample_doc_id:
            return self.sample_doc_id

        pdf_bytes = base64.b64decode(SAMPLE_PDF_B64)
        boundary = "----OxidBenchBoundary" + hex(random.randint(1000000, 9999999))
        body = io.BytesIO()
        body.write(f"--{boundary}\r\n".encode())
        body.write(b'Content-Disposition: form-data; name="file"; filename="sample_bench.pdf"\r\n')
        body.write(b"Content-Type: application/pdf\r\n\r\n")
        body.write(pdf_bytes)
        body.write(f"\r\n--{boundary}--\r\n".encode())
        payload = body.getvalue()

        headers = {
            "Content-Type": f"multipart/form-data; boundary={boundary}",
            "Content-Length": str(len(payload)),
        }
        if self.api_key:
            headers["X-API-Key"] = self.api_key

        try:
            conn = self.get_connection()
            conn.request("POST", "/api/documents/upload", body=payload, headers=headers)
            resp = conn.getresponse()
            raw_body = resp.read().decode("utf-8")
            conn.close()
            if resp.status == 200:
                import json
                data = json.loads(raw_body)
                self.sample_doc_id = data.get("id")
                print(f"[INFO] Document de benchmark initialisé : {CYAN}{self.sample_doc_id}{RESET} (2 pages)")
                return self.sample_doc_id
            else:
                print(f"{YELLOW}[WARN] Échec upload document (HTTP {resp.status} : {raw_body}){RESET}")
        except Exception as e:
            print(f"{YELLOW}[WARN] Erreur upload document : {e}{RESET}")

        # ID de secours par défaut
        self.sample_doc_id = "sample-doc-1"
        return self.sample_doc_id

    def run_worker_loop(self, task_func, stop_time: float, thread_id: int):
        latencies = []
        bytes_count = 0
        successes = 0
        errors = 0

        conn = self.get_connection()

        while time.time() < stop_time:
            method, path, headers, body, expected_status = task_func(thread_id)
            if self.api_key and "X-API-Key" not in headers:
                headers["X-API-Key"] = self.api_key

            t0 = time.perf_counter()
            try:
                conn.request(method, path, body=body, headers=headers)
                resp = conn.getresponse()
                data = resp.read()
                elapsed_ms = (time.perf_counter() - t0) * 1000.0

                if resp.status == expected_status or (expected_status == 200 and resp.status < 400):
                    successes += 1
                    bytes_count += len(data)
                    latencies.append(elapsed_ms)
                else:
                    errors += 1
            except Exception:
                errors += 1
                try:
                    conn.close()
                except Exception:
                    pass
                conn = self.get_connection()

        try:
            conn.close()
        except Exception:
            pass

        return successes, errors, bytes_count, latencies

    def execute_scenario(self, name: str, task_factory, duration_secs: int, concurrency: int) -> BenchmarkResult:
        result = BenchmarkResult(name)
        print(f"\n{BOLD}Démarrage du test : {CYAN}{name}{RESET}")
        print(f"  * Cible       : {self.target_url}")
        print(f"  * Concurrence : {concurrency} workers Keep-Alive")
        print(f"  * Durée       : {duration_secs} secondes")

        stop_time = time.time() + duration_secs
        t_start = time.perf_counter()

        with ThreadPoolExecutor(max_workers=concurrency) as executor:
            futures = [
                executor.submit(self.run_worker_loop, task_factory, stop_time, tid)
                for tid in range(concurrency)
            ]
            for f in as_completed(futures):
                s, e, b, l = f.result()
                result.success_count += s
                result.error_count += e
                result.bytes_transferred += b
                result.latencies_ms.extend(l)

        result.duration_secs = time.perf_counter() - t_start
        result.print_summary()
        return result

    # --------------------------------------------------------------------------
    # Scénarios de tests
    # --------------------------------------------------------------------------

    def scenario_gateway(self, duration_secs: int, concurrency: int):
        headers = {"Accept": "application/json"}
        def task(_tid):
            return "GET", "/api/health", headers.copy(), None, 200
        return self.execute_scenario("1. Gateway & API Throughput (/api/health)", task, duration_secs, concurrency)

    def scenario_cache_hit(self, duration_secs: int, concurrency: int):
        doc_id = self.ensure_sample_document()
        path = f"/api/documents/{doc_id}/pages/1/render?dpi=120"

        # Préchauffage initial (warmup 1 requête)
        try:
            c = self.get_connection()
            h = {}
            if self.api_key: h["X-API-Key"] = self.api_key
            c.request("GET", path, headers=h)
            c.getresponse().read()
            c.close()
        except Exception:
            pass

        headers = {"Accept": "image/jpeg,image/png"}
        def task(_tid):
            return "GET", path, headers.copy(), None, 200
        return self.execute_scenario("2. Consultation Haute Vitesse (Cache Hit L1/L2)", task, duration_secs, concurrency)

    def scenario_cold_rendering(self, duration_secs: int, concurrency: int):
        doc_id = self.ensure_sample_document()
        headers = {"Accept": "image/jpeg,image/png"}

        def task(_tid):
            dpi = random.randint(72, 180)
            salt = random.randint(10000, 999999)
            path = f"/api/documents/{doc_id}/pages/1/render?dpi={dpi}&s={salt}"
            return "GET", path, headers.copy(), None, 200

        return self.execute_scenario("3. Rendu A Froid / Calcul CPU Brut (Zero Cache Rasterisation)", task, duration_secs, concurrency)

    def scenario_convert_oneshot(self, duration_secs: int, concurrency: int):
        boundary = "----OxidBenchConvBoundary"
        body = io.BytesIO()
        body.write(f"--{boundary}\r\n".encode())
        body.write(b'Content-Disposition: form-data; name="file"; filename="document.txt"\r\n')
        body.write(b"Content-Type: text/plain\r\n\r\n")
        body.write(b"Hello world from Oxid Benchmark suite. Conversion pipeline test.\n" * 5)
        body.write(f"\r\n--{boundary}--\r\n".encode())
        payload = body.getvalue()

        headers = {
            "Content-Type": f"multipart/form-data; boundary={boundary}",
            "Content-Length": str(len(payload)),
        }

        def task(_tid):
            return "POST", "/api/convert", headers.copy(), payload, 200

        return self.execute_scenario("4. Ingestion & Conversion A La Volee (POST /api/convert)", task, duration_secs, concurrency)

    def scenario_production_blend(self, duration_secs: int, concurrency: int):
        doc_id = self.ensure_sample_document()

        def task(_tid):
            r = random.random()
            if r < 0.70:
                # 70% Cache Hit
                return "GET", f"/api/documents/{doc_id}/pages/1/render?dpi=120", {"Accept": "image/jpeg"}, None, 200
            elif r < 0.85:
                # 15% Thumbnail
                return "GET", f"/api/documents/{doc_id}/pages/1/thumbnail", {"Accept": "image/jpeg"}, None, 200
            elif r < 0.95:
                # 10% Cold Render (DPI variable)
                dpi = random.randint(90, 150)
                salt = random.randint(1000, 99999)
                return "GET", f"/api/documents/{doc_id}/pages/1/render?dpi={dpi}&s={salt}", {"Accept": "image/jpeg"}, None, 200
            else:
                # 5% Health Check
                return "GET", "/api/health", {"Accept": "application/json"}, None, 200

        return self.execute_scenario("5. Mixte Reel de Production (70% Cache, 15% Thumb, 10% Cold, 5% Health)", task, duration_secs, concurrency)


def main():
    parser = argparse.ArgumentParser(
        description="Oxid Benchmark & Performance Stress-Test Client",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Exemples :
  # Benchmark local complet :
  python3 oxid_bench.py --target http://localhost:8080 --scenario all --duration 10 -c 30

  # Exécution depuis un autre poste du réseau :
  python3 oxid_bench.py --target http://192.168.2.142:8080 --scenario mixed --duration 30 -c 50

  # Test avec clé API et quota augmenté :
  python3 oxid_bench.py --target http://192.168.2.142:8080 --api-key sk_bench --scenario convert
        """
    )
    parser.add_argument("--target", default="http://localhost:8080", help="URL cible d'Oxid (défaut: http://localhost:8080)")
    parser.add_argument("-c", "--concurrency", type=int, default=20, help="Nombre de connexions concurrentes Keep-Alive (défaut: 20)")
    parser.add_argument("-d", "--duration", type=int, default=10, help="Durée de chaque test en secondes (défaut: 10)")
    parser.add_argument("-s", "--scenario", choices=["all", "gateway", "cache", "cold", "convert", "mixed"], default="all", help="Scénario à tester")
    parser.add_argument("--doc-id", default="", help="ID d'un document existant sur le serveur (évite l'upload initial)")
    parser.add_argument("--api-key", default="", help="Clé API optionnelle (transmise dans X-API-Key)")
    parser.add_argument("-v", "--verbose", action="store_true", help="Afficher les détails de requêtes")

    args = parser.parse_args()

    print(f"{BOLD}================================================================================{RESET}")
    print(f"            {BOLD}{CYAN}OXID BENCHMARK SUITE — MÉTROLOGIE DE PRODUCTION{RESET}            ")
    print(f"{BOLD}================================================================================{RESET}")
    print(f"  * Cible        : {CYAN}{args.target}{RESET}")
    print(f"  * Concurrence  : {args.concurrency} workers persistants")
    print(f"  * Durée test   : {args.duration} s par scénario")
    print(f"  * Scénario     : {args.scenario}")
    print(f"  * Clé API      : {'Configurée' if args.api_key else 'Mode anonyme/public'}")
    print(f"{BOLD}================================================================================{RESET}\n")

    client = OxidBenchClient(args.target, args.api_key, args.verbose, args.doc_id)

    print(f"[1/2] Test de connectivité vers {args.target}...")
    if not client.check_health():
        print(f"{RED}[FATAL] Le serveur Oxid ne répond pas sur {args.target}/api/health.{RESET}")
        print("Assurez-vous qu'Oxid écoute sur 0.0.0.0:8080 et que le port est accessible.")
        sys.exit(1)
    print(f"{GREEN}[OK] Serveur Oxid opérationnel et accessible.{RESET}\n")

    print(f"[2/2] Exécution des scénarios...")
    results = []

    if args.scenario in ("all", "gateway"):
        results.append(client.scenario_gateway(args.duration, args.concurrency))

    if args.scenario in ("all", "cache"):
        results.append(client.scenario_cache_hit(args.duration, args.concurrency))

    if args.scenario in ("all", "cold"):
        results.append(client.scenario_cold_rendering(args.duration, min(args.concurrency, 10)))

    if args.scenario in ("all", "convert"):
        results.append(client.scenario_convert_oneshot(args.duration, min(args.concurrency, 10)))

    if args.scenario in ("all", "mixed"):
        results.append(client.scenario_production_blend(args.duration, args.concurrency))

    # Synthèse générale
    print(f"\n\n{BOLD}================================================================================")
    print(f"                     SYNTHÈSE GLOBALE DE LA CAMPAGNE                         ")
    print(f"================================================================================{RESET}")
    print(f"{'Scénario':<45} | {'Débit (req/s)':<14} | {'p50 (ms)':<9} | {'p99 (ms)':<9} | {'Erreurs'}")
    print("-" * 88)
    for r in results:
        r.latencies_ms.sort()
        p50 = r.percentile(50)
        p99 = r.percentile(99)
        print(f"{r.name:<45} | {r.rps:>12,.1f}  | {p50:>7.2f} ms | {p99:>7.2f} ms | {r.error_count}")
    print(f"{BOLD}================================================================================{RESET}\n")


if __name__ == "__main__":
    main()
