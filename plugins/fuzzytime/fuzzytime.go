//
// @package subtle-rs
//
// @file Fuzzytime plugin functions
// @copyright (c) 2025-present Christoph Kappel <christoph@unexist.dev>
// @version $Id$
//
// This program can be distributed under the terms of the GNU GPLv3.
// See the file LICENSE for details.
//

package main

import (
	"github.com/extism/go-pdk"
	"strconv"
	"strings"
	"math"
)

var numbers = []string {
	 "one", "two", "three", "four", "five", "six",
	 "seven", "eight", "nine", "ten", "eleven", "twelve",
}

var text = []string {
	 "%0 o'clock", "five past %0", "ten past %0", "quarter past %0",
	 "twenty past %0", "twenty-five past %0", "half past %0",
	 "twenty-five to %1", "twenty to %1", "quarter to %1", "ten to %1",
	 "five to %1", "%1 o'clock",
}

//go:wasmimport extism:host/user get_formatted_time
func get_formatted_time(uint64) uint64

//go:export run
func Run() int32 {
	format := "[hour repr:12]:[minute]"
	mem := pdk.AllocateString(format)
	defer mem.Free()

	ptr := get_formatted_time(mem.Offset())
	rmem := pdk.FindMemory(ptr)

	fields := strings.Split(string(rmem.ReadBytes()), ":")

	hour, _ := strconv.Atoi(fields[0])
	minute, _ := strconv.Atoi(fields[1])

	result := strings.ReplaceAll(text[int(math.Round(float64(minute) / 5))], "%0", numbers[hour - 1])

	cur_hour := hour
	if 12 == hour { cur_hour = 0 }

	result = strings.ReplaceAll(result, "%1", numbers[cur_hour])

	pdk.OutputString(result)

	return 0
}

func main() {}

