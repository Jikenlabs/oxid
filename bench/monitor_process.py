#!/usr/bin/env python3
"""
Oxid Process & Cluster System Telemetry Monitor
Surveille en temps réel un ou plusieurs nœuds Oxid (Cluster) pendant un stress-test réseau :
- Consommation CPU cumulée (% utilisateur et noyau par nœud et total)
- Mémoire vive : VmRSS, VmPeak, VmHWM
- Descripteurs de fichiers ouverts (FDs)
- Sockets TCP actives (ESTABLISHED, TIME_WAIT sur les ports 8080, 8081, 8083)
Génère une télémétrie continue à l'écran et exporte les métriques dans un fichier CSV.
"""

import argparse
import csv
import datetime
import os
import subprocess
import sys
import time

# Couleurs ANSI
GREEN = "\033[92m"
YELLOW = "\033[93m"
RED = "\033[91m"
CYAN = "\033[96m"
BOLD = "\033[1m"
RESET = "\033[0m"


def find_oxid_pids() -> list[int]:
    pids = []
    try:
        out = subprocess.check_output(["pgrep", "-f", "target/release/oxid"]).decode().strip()
        for p in out.split():
            try:
                p_int = int(p)
                if p_int not in pids and p_int != os.getpid():
                    pids.append(p_int)
            except ValueError:
                pass
    except Exception:
        pass
    return sorted(pids)


def read_proc_status(pid: int) -> dict:
    data = {}
    path = f"/proc/{pid}/status"
    if not os.path.exists(path):
        return data
    with open(path, "r") as f:
        for line in f:
            parts = line.split(":")
            if len(parts) >= 2:
                key = parts[0].strip()
                val = parts[1].strip()
                data[key] = val
    return data


def read_proc_stat(pid: int):
    path = f"/proc/{pid}/stat"
    if not os.path.exists(path):
        return 0, 0
    with open(path, "r") as f:
        parts = f.read().split()
        if len(parts) >= 15:
            return int(parts[13]), int(parts[14])
    return 0, 0


def count_open_fds(pid: int) -> int:
    path = f"/proc/{pid}/fd"
    try:
        return len(os.listdir(path))
    except Exception:
        return 0


def count_tcp_sockets(ports: list[int] = [8080, 8081, 8083]):
    established = 0
    time_wait = 0
    try:
        port_filter = " or ".join([f"sport = :{p}" for p in ports])
        out = subprocess.check_output(["ss", "-t", "-a", port_filter], stderr=subprocess.DEVNULL).decode()
        for line in out.splitlines():
            if "ESTAB" in line:
                established += 1
            elif "TIME-WAIT" in line or "TIME_WAIT" in line:
                time_wait += 1
    except Exception:
        pass
    return established, time_wait


def main():
    parser = argparse.ArgumentParser(description="Oxid Cluster & Process Live Monitor during Benchmarks")
    parser.add_argument("--pids", nargs="*", type=int, default=[], help="PIDs des processus Oxid (auto-détectés par défaut)")
    parser.add_argument("--interval", type=float, default=1.0, help="Intervalle d'échantillonnage en secondes (défaut: 1.0)")
    parser.add_argument("--output", default="bench/metrics_telemetry.csv", help="Fichier de sortie CSV")
    args = parser.parse_args()

    pids = args.pids or find_oxid_pids()
    if not pids:
        print(f"{RED}[ERREUR] Aucun processus Oxid actif détecté.{RESET}")
        sys.exit(1)

    clk_tck = os.sysconf(os.sysconf_names['SC_CLK_TCK'])
    num_cpus = os.cpu_count() or 1

    print(f"{BOLD}================================================================================{RESET}")
    print(f"       {BOLD}{CYAN}OXID CLUSTER MONITOR — TÉLÉMÉTRIE EN DIRECT (2 NŒUDS + LB){RESET}       ")
    print(f"{BOLD}================================================================================{RESET}")
    print(f"  * Nœuds surveillés   : {BOLD}{GREEN}{', '.join(str(p) for p in pids)}{RESET} ({len(pids)} instances)")
    print(f"  * Cœurs CPU système  : {num_cpus}")
    print(f"  * Port Load Balancer : 8080 (NGINX -> 8081 & 8083)")
    print(f"  * Export CSV         : {CYAN}{args.output}{RESET}")
    print(f"{BOLD}================================================================================{RESET}\n")

    csv_file = open(args.output, "w", newline="")
    csv_writer = csv.writer(csv_file)
    csv_writer.writerow([
        "timestamp", "elapsed_s", "active_nodes", "total_cpu_percent",
        "total_rss_mb", "max_hwm_mb", "total_threads", "total_fds",
        "tcp_established", "tcp_time_wait"
    ])

    header = f"{'Heure':<8} | {'Écoulé':<7} | {'Nœuds':<5} | {'CPU Total':<10} | {'RAM Totale':<12} | {'Threads':<7} | {'FDs':<6} | {'TCP ESTAB':<9} | {'TIME_WAIT'}"
    print(BOLD + header + RESET)
    print("-" * len(header))

    t_start = time.time()
    prev_stats = {p: read_proc_stat(p) for p in pids}
    prev_time = time.time()

    records_cpu = []
    records_rss = []

    try:
        while True:
            time.sleep(args.interval)
            now = time.time()
            dt = now - prev_time

            active_pids = [p for p in pids if os.path.exists(f"/proc/{p}")]
            if not active_pids:
                print(f"\n{RED}[ALERTE] Tous les nœuds Oxid se sont arrêtés !{RESET}")
                break

            total_cpu = 0.0
            total_rss = 0.0
            max_hwm = 0.0
            total_threads = 0
            total_fds = 0

            for p in active_pids:
                cur_u, cur_s = read_proc_stat(p)
                prev_u, prev_s = prev_stats.get(p, (cur_u, cur_s))
                ticks = (cur_u - prev_u) + (cur_s - prev_s)
                p_cpu = (ticks / (clk_tck * dt)) * 100.0 if dt > 0 else 0.0
                total_cpu += p_cpu

                st = read_proc_status(p)
                rss_kb = int(st.get("VmRSS", "0 kB").split()[0])
                hwm_kb = int(st.get("VmHWM", "0 kB").split()[0])
                total_rss += (rss_kb / 1024.0)
                max_hwm = max(max_hwm, hwm_kb / 1024.0)
                total_threads += int(st.get("Threads", "1"))
                total_fds += count_open_fds(p)

                prev_stats[p] = (cur_u, cur_s)

            estab, tw = count_tcp_sockets([8080, 8081, 8083])
            elapsed = int(now - t_start)
            h_str = datetime.datetime.now().strftime("%H:%M:%S")

            records_cpu.append(total_cpu)
            records_rss.append(total_rss)

            cpu_color = GREEN if total_cpu < 100 else (YELLOW if total_cpu < 300 else RED)
            row = (
                f"{h_str:<8} | {elapsed:>5}s  | {len(active_pids):>5} | "
                f"{cpu_color}{total_cpu:>8.1f}%{RESET} | "
                f"{total_rss:>9.2f} Mo | {total_threads:>7} | {total_fds:>6} | "
                f"{estab:>9} | {tw:>9}"
            )
            print(row)

            csv_writer.writerow([
                h_str, elapsed, len(active_pids), round(total_cpu, 2),
                round(total_rss, 2), round(max_hwm, 2), total_threads,
                total_fds, estab, tw
            ])
            csv_file.flush()
            prev_time = now

    except KeyboardInterrupt:
        print(f"\n{YELLOW}[INFO] Arrêt de la surveillance.{RESET}")

    finally:
        csv_file.close()


if __name__ == "__main__":
    main()
