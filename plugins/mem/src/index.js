const { get_memory } = Host.getFunctions()

export function run() {
  const ptr = get_memory();
  const rmem = Memory.find(ptr);
  const mem = rmem.readString();

  Host.outputString(`${mem}`)
}
