package rpc

import (
	"encoding/json"
	"net/http"
	"time"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/logger"
)

// HandleUnlock returns an http.HandlerFunc that processes an
// http.Request to unlock machine resources for a runner
//
// POST /rpc/v1/lock
func HandleUnlock(machines core.MachineStore) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		log := logger.FromRequest(r)
		machine, _ := MachineFrom(r.Context())

		in := new(core.Runner)
		err := json.NewDecoder(r.Body).Decode(in)
		if err != nil {
			writeError(w, err)
			log.WithError(err).
				Debugln("manager: cannot decode lock request payload")
			return
		}

		now := time.Now()

		machine.CPUAllocated -= in.CPU
		machine.RAMAllocated -= in.RAM
		machine.Updated = now.Unix()

		err = machines.Update(r.Context(), machine)
		if err != nil {
			writeError(w, err)
			log.WithError(err).
				Debugln("manager: cannot unlock machine resources")
			return
		}

		writeOK(w)
	}
}
