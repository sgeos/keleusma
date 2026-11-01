- **A host-built value of a generic struct instance is read by its instance layout.** A
  parameter typed with a generic struct instance that the program never constructs kept the
  template's name, so a field read was baked as an opaque handle and a host-built argument read
  wrong values or faulted. Monomorphization now specializes every concrete generic struct
  instance a signature names. Pinned by `tests/generic_instance_host_values.rs`. A pykeleusma
  finding.
