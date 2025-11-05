package machine

import (
	"context"
	"testing"
	"time"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/store/shared/db/dbtest"
)

var noContext = context.TODO()

func TestMachine(t *testing.T) {
	conn, err := dbtest.Connect()
	if err != nil {
		t.Error(err)
		return
	}
	defer func() {
		dbtest.Reset(conn)
		dbtest.Disconnect(conn)
	}()

	store := New(conn)
	t.Run("Create", testMachineCreate(store))
}

func testMachineCreate(store core.MachineStore) func(t *testing.T) {
	return func(t *testing.T) {
		now := time.Now().Unix()
		machine := &core.Machine{
			Name:     "machine-test-001",
			Owner:    "testorg",
			Arch:     "amd64",
			CPU:      4,
			RAM:      8192,
			Status:   core.MachineStatusOnline,
			Created:  now,
			LastSeen: now,
			Updated:  now,
			Token:    "token-abc123def456",
		}
		err := store.Create(noContext, machine)
		if err != nil {
			t.Error(err)
		}

		t.Run("Find", testMachineFind(store, machine))
		t.Run("FindToken", testMachineFindToken(store, machine))
		t.Run("List", testMachineList(store, machine))
		t.Run("Update", testMachineUpdate(store, machine))
		t.Run("Purge", testMachinePurge(store, machine))
		t.Run("Delete", testMachineDelete(store, machine))
	}
}

func testMachineFind(store core.MachineStore, created *core.Machine) func(t *testing.T) {
	return func(t *testing.T) {
		machine, err := store.Find(noContext, created.Owner, created.Name)
		if err != nil {
			t.Error(err)
		} else {
			t.Run("Fields", testMachineFields(machine, created))
		}
	}
}

func testMachineFindToken(store core.MachineStore, created *core.Machine) func(t *testing.T) {
	return func(t *testing.T) {
		machine, err := store.FindToken(noContext, created.Token)
		if err != nil {
			t.Error(err)
		} else if machine.Token != created.Token {
			t.Errorf("Want token %s, got %s", created.Token, machine.Token)
		}
	}
}

func testMachineList(store core.MachineStore, created *core.Machine) func(t *testing.T) {
	return func(t *testing.T) {
		machines, err := store.List(noContext, created.Owner)
		if err != nil {
			t.Error(err)
		} else if len(machines) < 1 {
			t.Errorf("Want machines list length >= 1, got %d", len(machines))
		}
	}
}

func testMachineUpdate(store core.MachineStore, created *core.Machine) func(t *testing.T) {
	return func(t *testing.T) {
		created.Status = core.MachineStatusOffline
		created.LastSeen = time.Now().Unix()
		created.Updated = time.Now().Unix()

		err := store.Update(noContext, created)
		if err != nil {
			t.Error(err)
		}

		machine, err := store.Find(noContext, created.Owner, created.Name)
		if err != nil {
			t.Error(err)
		} else if machine.Status != core.MachineStatusOffline {
			t.Errorf("Want status %s, got %s", core.MachineStatusOffline, machine.Status)
		}
	}
}

func testMachinePurge(store core.MachineStore, created *core.Machine) func(t *testing.T) {
	return func(t *testing.T) {
		// Create an old machine
		oldMachine := &core.Machine{
			Name:     "old-machine-001",
			Owner:    created.Owner,
			Arch:     "amd64",
			CPU:      2,
			RAM:      4096,
			Status:   core.MachineStatusOffline,
			Created:  1000000000,
			LastSeen: 1000000000,
			Updated:  1000000000,
			Token:    "old-token-xyz",
		}
		err := store.Create(noContext, oldMachine)
		if err != nil {
			t.Error(err)
		}

		// Purge machines with last_seen before 1500000000
		err = store.Purge(noContext, 1500000000)
		if err != nil {
			t.Error(err)
		}

		// Verify old machine is gone
		_, err = store.Find(noContext, oldMachine.Owner, oldMachine.Name)
		if err == nil {
			t.Error("Expected error when finding purged machine")
		}

		// Verify created machine still exists (it has recent last_seen)
		_, err = store.Find(noContext, created.Owner, created.Name)
		if err != nil {
			t.Error(err)
		}
	}
}

func testMachineDelete(store core.MachineStore, created *core.Machine) func(t *testing.T) {
	return func(t *testing.T) {
		err := store.Delete(noContext, created)
		if err != nil {
			t.Error(err)
		}

		_, err = store.Find(noContext, created.Owner, created.Name)
		if err == nil {
			t.Error("Expected error when finding deleted machine")
		}
	}
}

func testMachineFields(got *core.Machine, want *core.Machine) func(t *testing.T) {
	return func(t *testing.T) {
		if got.Name != want.Name {
			t.Errorf("Want name %s, got %s", want.Name, got.Name)
		}
		if got.Owner != want.Owner {
			t.Errorf("Want owner %s, got %s", want.Owner, got.Owner)
		}
		if got.Arch != want.Arch {
			t.Errorf("Want arch %s, got %s", want.Arch, got.Arch)
		}
		if got.CPU != want.CPU {
			t.Errorf("Want cpu %d, got %d", want.CPU, got.CPU)
		}
		if got.RAM != want.RAM {
			t.Errorf("Want ram %d, got %d", want.RAM, got.RAM)
		}
		if got.Status != want.Status {
			t.Errorf("Want status %s, got %s", want.Status, got.Status)
		}
		if got.Token != want.Token {
			t.Errorf("Want token %s, got %s", want.Token, got.Token)
		}
	}
}
