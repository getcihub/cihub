package rpc

import (
	"context"

	"github.com/getcihub/cihub/core"
)

type key int

const machineKey key = iota

// WithMachine returns a copy of parent in which the machine value is set
func WithMachine(parent context.Context, machine *core.Machine) context.Context {
	return context.WithValue(parent, machineKey, machine)
}

// MachineFrom returns the value of the machine key on the ctx
func MachineFrom(ctx context.Context) (*core.Machine, bool) {
	machine, ok := ctx.Value(machineKey).(*core.Machine)
	return machine, ok
}
