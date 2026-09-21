"""Prints the capital/commons data-legibility finding. No network calls —
this is a static assessment of what was tested while building the pipeline,
not a live measurement, so it runs anywhere sbci/ imports."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from sbci.data_fabric import assess_data_fabric, print_fabric_report  # noqa: E402

if __name__ == "__main__":
    print_fabric_report(assess_data_fabric())
