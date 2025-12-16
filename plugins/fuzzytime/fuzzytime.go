package main

import (
	"github.com/extism/go-pdk"
)

//go:wasmimport extism:host/user get_formatted_time
func get_formatted_time(uint64) uint64

//go:export run
func Run() int32 {
	format := "[hour]:[minute]:[second]"
	mem := pdk.AllocateString(format)
	defer mem.Free()

	ptr := get_formatted_time(mem.Offset())
	rmem := pdk.FindMemory(ptr)

	pdk.OutputString(string(rmem.ReadBytes()))

	return 0
}

func main() {}

