#!/usr/bin/env python3
"""
Oxid Process & System Telemetry Monitor
Surveille en temps réel l'état du processus Oxid pendant un stress-test réseau :
- Consommation CPU (% utilisateur et noyau)
- Mémoire vive : VmRSS, VmPeak, VmHWM
- Descripteurs de fichiers ouverts (FDs)
- Sockets TCP actives (ESTABLISHED, TIME_WAIT)
- Changements de contexte (volontaires / involontaires)
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


def find_oxid_pid() -> int:
    try:
        out = subprocess.check_output(["pgrep", "-f", "target/release/oxid"]).decode().strip()
        lines = out.split()
        if lines:
            return int(lines[0])
    except Exception:
        pass
    try:
        out = subprocess.check_output(["pgrep", "-f", "oxid"]).decode().strip()
        for pid in out.split():
            cmdline = open(f"/proc/{pid}/cmdline", "r").read()
            if "target/release/oxid" in cmdline or "oxid" in cmdline:
                return int(pid)
    except Exception:
        pass
    return 0


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
        # utime = parts[13], stime = parts[14]
        if len(parts) >= 15:
            return int(parts[13]), int(parts[14])
    return 0, 0


def count_open_fds(pid: int) -> int:
    path = f"/proc/{pid}/fd"
    try:
        return len(os.listdir(path))
    except Exception:
        return 0


def count_tcp_sockets(port: int = 8080):
    established = 0
    time_wait = 0
    try:
        out = subprocess.check_output(["ss", "-t", "-a", f"sport = :{port}"], stderr=subprocess.DEVNULL).decode()
        for line in out.splitlines():
            if "ESTAB" in line:
                established += 1
            elif "TIME-WAIT" in line or "TIME_WAIT" in line:
                time_wait += 1
    except Exception:
        pass
    return established, time_wait


def main():
    parser = argparse.ArgumentParser(description="Oxid Process Live Monitor during Benchmarks")
    parser.add_argument("--pid", type=int, default=0, help="PID du processus Oxid (auto-détecté par défaut)")
    parser.add_argument("--interval", type=float, default=1.0, help="Intervalle d'échantillonnage en secondes (défaut: 1.0)")
    parser.add_argument("--output", default="bench/metrics_telemetry.csv", help="Fichier de sortie CSV")
    parser.add_argument("--port", type=int, default=8080, help="Port d'écoute HTTP d'Oxid (défaut: 8080)")
    args = parser.parse_args()

    pid = args.pid or find_oxid_pid()
    if not pid or not os.path.exists(f"/proc/{pid}"):
        print(f"{RED}[ERREUR] Impossible de trouver le processus Oxid en cours d'exécution.{RESET}")
        print("Veuillez spécifier explicitement le PID avec --pid <PID>.")
        sys.exit(1)

    clk_tck = os.sysconf(os.sysconf_names['SC_CLK_TCK'])
    num_cpus = os.cpu_count() or 1

    print(f"{BOLD}================================================================================{RESET}")
    print(f"            {BOLD}{CYAN}OXID MONITOR — TÉLÉMÉTRIE DU PROCESSUS EN DIRECT{RESET}            ")
    print(f"{BOLD}================================================================================{RESET}")
    print(f"  * PID surveillé      : {BOLD}{GREEN}{pid}{RESET}")
    print(f"  * Intervalle mesure  : {args.interval} s")
    print(f"  * Nombre de cœurs CPU: {num_cpus}")
    print(f"  * Fichier export CSV : {CYAN}{args.output}{RESET}")
    print(f"{BOLD}================================================================================{RESET}\n")

    csv_file = open(args.output, "w", newline="")
    csv_writer = csv.writer(csv_file)
    csv_writer.writerow([
        "timestamp", "elapsed_s", "cpu_percent", "vm_rss_mb", "vm_hwm_mb",
        "threads", "open_fds", "tcp_established", "tcp_time_wait"
    ])

    header = f"{'Heure':<8} | {'Écoulé':<7} | {'CPU (%)':<8} | {'RAM (RSS)':<10} | {'Pic (HWM)':<10} | {'Threads':<7} | {'FDs':<6} | {'TCP ESTAB':<9} | {'TIME_WAIT'}"
    print(BOLD + header + RESET)
    print("-" * len(header))

    t_start = time.time()
    prev_utime, prev_stime = read_proc_stat(pid)
    prev_time = time.time()

    records_cpu = []
    records_rss = []
    records_fds = []
    records_estab = []

    try:
        while True:
            time.sleep(args.interval)
            now = time.time()
            dt = now - prev_time

            cur_utime, cur_stime = read_proc_stat(pid)
            if not os.path.exists(f"/proc/{pid}"):
                print(f"\n{RED}[ALERTE] Le processus {pid} s'est arrêté !{RESET}")
                break

            status = read_proc_status(pid)

            # Calcul CPU %
            total_ticks = (cur_utime - prev_utime) + (cur_stime - prev_stime)
            cpu_percent = (total_ticks / (clk_tck * dt)) * 100.0 if dt > 0 else 0.0

            # Calcul RAM
            rss_kb = int(status.get("VmRSS", "0 kB").split()[0])
            hwm_kb = int(status.get("VmHWM", "0 kB").split()[0])
            rss_mb = rss_kb / 1024.0
            hwm_mb = hwm_kb / 1024.0

            threads = int(status.get("Threads", "1"))
            fds = count_open_fds(pid)
            estab, tw = count_tcp_sockets(args.port)

            elapsed = int(now - t_start)
            h_str = datetime.datetime.now().strftime("%H:%M:%S")

            records_cpu.append(cpu_percent)
            records_rss.append(rss_mb)
            records_fds.append(fds)
            records_estab.append(estab)

            # Ligne console
            cpu_color = GREEN if cpu_percent < 50 else (YELLOW if cpu_percent < 150 else RED)
            row = (
                f"{h_str:<8} | {elapsed:>5}s  | {cpu_color}{cpu_percent:>7.1f}%{RESET} | "
                f"{rss_mb:>7.2f} Mo | {hwm_mb:>7.2f} Mo | {threads:>7} | {fds:>6} | "
                f"{estab:>9} | {tw:>9}"
            )
            print(row)

            # CSV Write
            csv_writer.writerow([
                h_str, elapsed, round(cpu_percent, 2), round(rss_mb, 2), round(hwm_mb, 2),
                threads, fds, estab, tw
            ])
            csv_file.flush()

            prev_utime, prev_stime = cur_utime, cur_stime
            prev_time = now

    except KeyboardInterrupt:
        print(f"\n{YELLOW}[INFO] Arrêt de la surveillance.{RESET}")

    finally:
        csv_file.close()

    # Synthèse
    if records_cpu:
        print(f"\n{BOLD}================================================================================")
        print(f"                   SYNTHÈSE DE LA TÉLÉMÉTRIE SYSTÈME                         ")
        print(f"================================================================================{RESET}")
        print(f"  * Durée d'observation      : {int(time.time() - t_start)} secondes")
        print(f"  * CPU Utilisé (Moyenne)    : {sum(records_cpu)/len(records_cpu):.1f}%")
        print(f"  * CPU Utilisé (Pic Max)    : {max(records_cpu):.1f}%")
        print(f"  * RAM Résidente Initiale   : {records_rss[0]:.2f} Mo")
        print(f"  * RAM Résidente Finale     : {records_rss[-1]:.2f} Mo (Variation : {records_rss[-1] - records_rss[0]:+.2f} Mo)")
        print(f"  * Pic Historique RAM (HWM) : {max(records_rss):.2f} Mo")
        print(f"  * Descripteurs FDs Max     : {max(records_fds)}")
        print(f"  * Connexions TCP Max simul.: {max(records_estab)} actives simultanées")
        print(f"  * Fichier de logs détaillé : {CYAN}{args.output}{RESET}")
        print(f"{BOLD}================================================================================{RESET}\n")


if __name__ == "__main__":
    main()
