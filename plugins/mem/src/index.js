const { get_memory } = Host.getFunctions()

export function run() {
  const ptr = get_memory();
  const rmem = Memory.find(ptr);

  const mem = rmem.readString().split(", ")
      .flatMap((v) => Math.round(parseInt(v) / 1024 / 1024));

  Host.outputString(`${mem[0]}g / ${mem[2]}g`)
}
