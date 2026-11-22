package app.pocketbridge.bridge.data

/** In-memory [Store] for JVM tests. */
class MemoryStore : Store {
    private val map = mutableMapOf<String, String>()

    override fun putString(key: String, value: String) {
        map[key] = value
    }

    override fun getString(key: String): String? = map[key]

    override fun remove(key: String) {
        map.remove(key)
    }
}
