package com.example.clipboardsync

import android.content.Context
import androidx.room.ColumnInfo
import androidx.room.Dao
import androidx.room.Database
import androidx.room.Delete
import androidx.room.Entity
import androidx.room.Insert
import androidx.room.PrimaryKey
import androidx.room.Query
import androidx.room.Room
import androidx.room.RoomDatabase
import androidx.room.Update
import java.security.PublicKey

@Database(entities = [Peer::class], version = 1)
abstract class MainDatabase : RoomDatabase() {
    abstract fun peerDao(): PeerDao

    companion object {
        @Volatile
        var instance: MainDatabase? = null

        fun init(context: Context): MainDatabase? {
            this.instance = Room.databaseBuilder(
                context,
                MainDatabase::class.java,
                "database"
            ).build()
            return this.instance
        }
    }
}

@Entity
data class Peer(
    @PrimaryKey(autoGenerate = false)
    val key: String
)

@Dao
interface PeerDao {
    @Query("SELECT * FROM peer")
    fun getAll(): List<Peer>

    @Insert
    fun insert(vararg peers: Peer)

    @Delete
    fun delete(peer: Peer)

    @Update
    fun update(peer: Peer)
}