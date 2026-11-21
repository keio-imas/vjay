use rusqlite::Connection;
use std::sync::Arc;
use vjay_application::ports::PortResult;
pub(super) struct Track {
    pub row: i64,
    pub bpm: f32,
    pub duration: f32,
    pub text: Vec<Arc<str>>,
    pub cover: Option<std::path::PathBuf>,
}
pub(super) fn latest(db: &Connection) -> PortResult<Option<Track>> {
    let has = |table: &str| -> bool {
        db.prepare(&format!("PRAGMA table_info({table})"))
            .and_then(|mut q| {
                let columns = q
                    .query_map([], |r| r.get::<_, String>(1))?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(columns.iter().any(|c| c == "ImagePath"))
            })
            .unwrap_or(false)
    };
    let cover = match (has("djmdContent"), has("djmdAlbum")) {
        (true, true) => "COALESCE(NULLIF(c.ImagePath,''),al.ImagePath)",
        (true, false) => "c.ImagePath",
        (false, true) => "al.ImagePath",
        _ => "NULL",
    };
    let sql=format!("SELECT h.rowid,c.BPM,c.Length,c.Title,a.Name,al.Name,g.Name,l.Name,k.ScaleName,c.FolderPath,c.ID,h.created_at,{cover}
        FROM djmdSongHistory h JOIN djmdContent c ON c.ID=h.ContentID
        LEFT JOIN djmdArtist a ON a.ID=c.ArtistID LEFT JOIN djmdAlbum al ON al.ID=c.AlbumID
        LEFT JOIN djmdGenre g ON g.ID=c.GenreID LEFT JOIN djmdLabel l ON l.ID=c.LabelID
        LEFT JOIN djmdKey k ON k.ID=c.KeyID
        WHERE COALESCE(h.rb_local_deleted,0)=0 AND COALESCE(c.rb_local_deleted,0)=0
        ORDER BY h.rowid DESC LIMIT 1");
    let mut query = db
        .prepare(&sql)
        .map_err(|e| format!("Unsupported rekordbox history schema: {e}"))?;
    let mut rows = query.query([]).map_err(|e| e.to_string())?;
    let Some(row) = rows.next().map_err(|e| e.to_string())? else {
        return Ok(None);
    };
    let mut text = Vec::new();
    for index in 3..12 {
        let value = row.get_ref(index).map_err(|e| e.to_string())?;
        let mut value = match value {
            rusqlite::types::ValueRef::Text(v) => String::from_utf8_lossy(v).into_owned(),
            rusqlite::types::ValueRef::Integer(v) => v.to_string(),
            _ => String::new(),
        };
        let mut end = value.len().min(16384);
        while !value.is_char_boundary(end) {
            end -= 1;
        }
        value.truncate(end);
        text.push(Arc::from(value));
    }
    text.push(Arc::from(
        "Latest history entry; playback detection is delayed",
    ));
    Ok(Some(Track {
        row: row.get(0).map_err(|e| e.to_string())?,
        bpm: row
            .get::<_, Option<f64>>(1)
            .unwrap_or_default()
            .unwrap_or(0.0) as f32
            / 100.0,
        duration: row
            .get::<_, Option<f64>>(2)
            .unwrap_or_default()
            .unwrap_or(0.0) as f32,
        text,
        cover: row
            .get::<_, Option<String>>(12)
            .ok()
            .flatten()
            .filter(|s| !s.is_empty())
            .map(std::path::PathBuf::from),
    }))
}
