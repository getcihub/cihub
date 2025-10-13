package dbtest

import (
	"os"
	"strconv"

	"github.com/getcihub/cihub/store/shared/db"

	_ "github.com/go-sql-driver/mysql"
	_ "github.com/lib/pq"
	_ "modernc.org/sqlite"
)

// Connect opens a new test database connection.
func Connect() (*db.DB, error) {
	var (
		driver         = db.Sqlite
		config         = ":memory:?_foreign_keys=1"
		maxConnections = 0
	)

	if os.Getenv("CIHUB_DATABASE_DRIVER") != "" {
		if err := driver.Set(os.Getenv("CIHUB_DATABASE_DRIVER")); err != nil {
			return nil, err
		}

		config = os.Getenv("CIHUB_DATABASE_DATASOURCE")
		maxConnectionsString := os.Getenv("CIHUB_DATABASE_MAX_CONNECTIONS")
		maxConnections, _ = strconv.Atoi(maxConnectionsString)
	}

	return db.Connect(driver, config, maxConnections)
}

// Disconnect closes the database connection.
func Disconnect(d *db.DB) error {
	return d.Close()
}

// Reset resets the database state.
func Reset(d *db.DB) {
	d.Lock(func(tx db.Execer, _ db.Binder) error {
		tx.Exec("DELETE FROM users")
		tx.Exec("DELETE FROM nodes")
		tx.Exec("DELETE FROM runners")
		tx.Exec("DELETE FROM jobs")
		return nil
	})
}
