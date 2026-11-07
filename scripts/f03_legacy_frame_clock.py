"""Independent physical clock reconstruction for the pinned historical population."""
from f03_verify_archived_oracle import require

MAX_GLOBAL = 65_536


def integer(payload, offset, size=8, *, signed=False):
    field = payload[offset:offset + size]
    require(len(field) == size, "truncated physical clock field")
    return int.from_bytes(field, "little", signed=signed)


class LegacyFrameClock:
    def __init__(self):
        self.head = 0
        self.reservations = set()
        self.times = {}

    def allocate(self, first, count, time):
        if count == 0:
            return
        require(first == self.head + 1 and self.head + count <= MAX_GLOBAL,
                "physical clock allocation is noncontiguous or exceeds the fixture bound")
        self.head += count
        if time is not None:
            self.times.update((version, str(time)) for version in range(first, self.head + 1))

    def consume(self, payload):
        marker = payload[:8]
        if marker in {b"CVAVERS1", b"CVAVERS2"}:
            timed = marker == b"CVAVERS2"
            require(len(payload) == (24 if timed else 16), "invalid standalone reservation length")
            version = integer(payload, 8)
            self.allocate(version, 1, integer(payload, 16, signed=True) if timed else None)
            self.reservations.add(version)
        elif marker == b"CVAPUB01":
            version = integer(payload, 8)
            mode = integer(payload, 16, 1)
            body_at = 25 if mode == 1 else 17
            require(mode in (0, 1, 2) and version > 0 and len(payload) >= body_at + 8
                    and not payload[body_at:].startswith(b"CVAPUB"), "invalid historical publication envelope")
            if mode == 2:
                require(version in self.reservations, "publication does not bind an unused standalone reservation")
                self.reservations.remove(version)
            else:
                self.allocate(version, 1, integer(payload, 17, signed=True) if mode == 1 else None)
        elif marker == b"CVAINSC8":
            shape = integer(payload, 8, 1)
            require(1 <= shape <= 6, "invalid retained Insomnia completion shape")
            if shape != 1:
                start = integer(payload, 65)
                count = integer(payload, 73, 4)
                time = integer(payload, 77, signed=True) if shape >= 4 else None
                self.allocate(start, count, time)
        elif marker.startswith((b"CVAINSC", b"CVAXMEM", b"CVAPUB", b"CVAVERS")):
            raise ValueError("clock carrier outside the pinned historical population")
        return self.head
