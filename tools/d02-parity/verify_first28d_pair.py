"""Verify the original first-28-day pair with the shared offline verifier."""

from verify_pair import FIRST28D, main


if __name__ == "__main__":
    raise SystemExit(main(default_pair=FIRST28D))
