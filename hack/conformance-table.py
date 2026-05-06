#!/usr/bin/env python3
# /// script
# requires-python = ">=3.8"
# ///
"""
Reads a conformance-tests.txt file (output of `make conformance-tests`)
and produces a markdown table summarising passed/failed/skipped counts
per test suite.

Usage:
    uv run hack/conformance-table.py conformance-tests.txt
    uv run hack/conformance-table.py conformance-tests.txt > report.md
"""

import re
import sys
from dataclasses import dataclass, field


@dataclass
class SuiteStats:
    name: str
    passed: int = 0
    failed: int = 0
    skipped: int = 0


def parse(path: str) -> list[SuiteStats]:
    suite_order: list[str] = []
    suites: dict[str, SuiteStats] = {}
    current: str | None = None

    # "Running conformance tests from: <name>" declares the suite being run.
    # "Conformance Test Results: <name>" precedes the PASSED/FAILED/SKIPPED summary
    # lines — use it to pin the current suite for counting.
    running_re = re.compile(r"Running conformance tests from:\s+(\S+)")
    results_re = re.compile(r"Conformance Test Results:\s+(\S+)")
    passed_re = re.compile(r"^PASSED:\s+(\d+)")
    failed_re = re.compile(r"^FAILED:\s+(\d+)")
    skipped_re = re.compile(r"^SKIPPED:\s+(\d+)")

    with open(path) as f:
        for raw in f:
            line = raw.strip()

            m = running_re.search(line)
            if m:
                name = m.group(1)
                if name not in suites:
                    suites[name] = SuiteStats(name=name)
                    suite_order.append(name)
                # Don't set current here; wait for the Results header.
                continue

            m = results_re.search(line)
            if m:
                current = m.group(1)
                if current not in suites:
                    suites[current] = SuiteStats(name=current)
                    suite_order.append(current)
                continue

            if current is None:
                continue

            m = passed_re.match(line)
            if m:
                suites[current].passed += int(m.group(1))
                continue

            m = failed_re.match(line)
            if m:
                suites[current].failed += int(m.group(1))
                continue

            m = skipped_re.match(line)
            if m:
                suites[current].skipped += int(m.group(1))

    return [suites[name] for name in suite_order]


def render(suites: list[SuiteStats]) -> str:
    if not suites:
        return "_No test suites found._\n"

    col_name = max(len("Test Suite"), max(len(s.name) for s in suites))
    col_pass = max(len("Successful"), max(len(str(s.passed)) for s in suites))
    col_fail = max(len("Failed"), max(len(str(s.failed)) for s in suites))
    col_skip = max(len("Skipped"), max(len(str(s.skipped)) for s in suites))

    def row(name: str, passed: str, failed: str, skipped: str) -> str:
        return (
            f"| {name:<{col_name}} "
            f"| {passed:>{col_pass}} "
            f"| {failed:>{col_fail}} "
            f"| {skipped:>{col_skip}} |"
        )

    sep = (
        f"| {'-' * col_name} "
        f"| {'-' * col_pass}:"
        f"| {'-' * col_fail}:"
        f"| {'-' * col_skip}:|"
    )

    lines = [
        row("Test Suite", "Successful", "Failed", "Skipped"),
        sep,
        *[row(s.name, str(s.passed), str(s.failed), str(s.skipped)) for s in suites],
    ]

    total_passed = sum(s.passed for s in suites)
    total_failed = sum(s.failed for s in suites)
    total_skipped = sum(s.skipped for s in suites)
    lines.append(row("**Total**", str(total_passed), str(total_failed), str(total_skipped)))

    return "\n".join(lines) + "\n"


def main() -> None:
    if len(sys.argv) != 2:
        print(f"Usage: {sys.argv[0]} <conformance-tests.txt>", file=sys.stderr)
        sys.exit(1)

    suites = parse(sys.argv[1])
    print(render(suites), end="")


if __name__ == "__main__":
    main()
