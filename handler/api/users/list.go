package users

import (
	"net/http"
	"strconv"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/handler/api/render"
	"github.com/getcihub/cihub/logger"
)

// HandleList returns an http.HandlerFunc that writes a json-encoded
// paginated list of users to the response body.
func HandleList(users core.UserStore) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		query := r.URL.Query()

		// Parse limit parameter (default: 20, max: 100)
		limit := 20
		if limitStr := query.Get("limit"); limitStr != "" {
			if parsedLimit, err := strconv.Atoi(limitStr); err == nil {
				limit = parsedLimit
				if limit > 100 {
					limit = 100
				}
				if limit < 1 {
					limit = 20
				}
			}
		}

		params := core.UserParams{
			After: query.Get("after"),
			Limit: limit,
		}

		userList, err := users.List(r.Context(), params)
		if err != nil {
			render.InternalError(w)
			logger.FromRequest(r).WithError(err).
				Warnln("api: cannot list users")
			return
		}

		// Check if there are more results
		hasMore := len(userList) > limit
		if hasMore {
			userList = userList[:limit]
		}

		render.Paginated(w, userList, hasMore)
	}
}
