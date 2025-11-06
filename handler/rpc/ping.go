package rpc

import (
	"encoding/json"
	"net/http"
	"time"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/logger"
)

// HandlePing returns an http.HandlerFunc that makes an
// http.Request to ping the server and confirm connectivity.
//
// POST /rpc/v1/ping
func HandlePing(machines core.MachineStore) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		machine, _ := MachineFrom(r.Context())

		in := new(core.Resource)
		err := json.NewDecoder(r.Body).Decode(in)
		if err != nil {
			logger.FromRequest(r).
				WithError(err).
				Debugln("rpc: cannot decode registering request payload")
			writeError(w, err)
			return
		}

		machine.Arch = in.Arch
		machine.CPU = in.CPU
		machine.LastSeen = time.Now().Unix()
		machine.RAMAvailable = in.RAMAvailable
		machine.RAMTotal = in.RAMTotal

		err = machines.Update(r.Context(), machine)
		if err != nil {
			writeError(w, err)
			logger.FromRequest(r).
				WithError(err).
				Debugln("rpc: cannot update machine")
			return
		}

		writeOK(w)
	}
}
