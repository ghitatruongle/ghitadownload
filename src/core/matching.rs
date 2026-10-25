const MIN_CANDIDATE_SCORE: f64 = 0.5;

const TITLE_WEIGHT: f64 = 0.5;
const ARTIST_WEIGHT: f64 = 0.2;
const DURATION_WEIGHT: f64 = 0.3;

const PLACEHOLDER_ARTISTS: &[&str] = &["raw", "search", "direct", "youtube", "spotify"];

const NOISE_PHRASES: &[&str] = &[
    "official music video",
    "music video",
    "official video",
    "official audio",
    "official mv",
    "lyrics video",
    "lyric video",
    "official visualizer",
];

const NOISE_TOKENS: &[&str] = &[
    "official",
    "video",
    "audio",
    "lyrics",
    "lyric",
    "mv",
    "hd",
    "4k",
    "remaster",
    "remastered",
    "feat",
    "ft",
    "featuring",
];

pub struct SearchCandidate {
    pub url: String,
    pub title: String,
    pub uploader: String,
    pub duration_secs: Option<f64>,
}

pub fn pick_best<'a>(
    candidates: &'a [SearchCandidate],
    expected_title: &str,
    artists: &[String],
    expected_duration_secs: Option<u64>,
) -> Option<&'a SearchCandidate> {
    let real_artists: Vec<&str> = artists
        .iter()
        .map(String::as_str)
        .filter(|artist| !is_placeholder_artist(artist))
        .collect();
    let mut best: Option<(f64, &SearchCandidate)> = None;
    for candidate in candidates {
        let score = score_candidate(
            candidate,
            expected_title,
            &real_artists,
            expected_duration_secs,
        );
        if score >= MIN_CANDIDATE_SCORE && best.is_none_or(|(best_score, _)| score > best_score) {
            best = Some((score, candidate));
        }
    }
    best.map(|(_, candidate)| candidate)
}

fn score_candidate(
    candidate: &SearchCandidate,
    expected_title: &str,
    artists: &[&str],
    expected_duration_secs: Option<u64>,
) -> f64 {
    let title_similarity = title_similarity(&candidate.title, expected_title, artists);
    let mut weighted = TITLE_WEIGHT * title_similarity;
    let mut total_weight = TITLE_WEIGHT;

    if let Some(artist_similarity) =
        artist_similarity(&candidate.title, &candidate.uploader, artists)
    {
        weighted += ARTIST_WEIGHT * artist_similarity;
        total_weight += ARTIST_WEIGHT;
    }

    if let Some(duration_similarity) =
        duration_similarity(candidate.duration_secs, expected_duration_secs)
    {
        weighted += DURATION_WEIGHT * duration_similarity;
        total_weight += DURATION_WEIGHT;
    }

    if total_weight <= 0.0 {
        return 0.0;
    }
    weighted / total_weight
}

fn title_similarity(candidate_title: &str, expected_title: &str, artists: &[&str]) -> f64 {
    let expected = core_tokens(expected_title, &[]);
    if expected.is_empty() {
        return 0.0;
    }
    let expected_joined = expected.join(" ");
    let candidate_full = core_tokens(candidate_title, &[]).join(" ");
    let candidate_without_artist = core_tokens(candidate_title, artists).join(" ");
    let full = dice_bigrams(&candidate_full, &expected_joined);
    let stripped = dice_bigrams(&candidate_without_artist, &expected_joined);
    let mut best = full.max(stripped);
    if expected_joined.chars().count() >= 4
        && (candidate_full.contains(&expected_joined)
            || candidate_without_artist.contains(&expected_joined))
    {
        best = best.max(0.9);
    }
    best
}

fn artist_similarity(candidate_title: &str, uploader: &str, artists: &[&str]) -> Option<f64> {
    if artists.is_empty() {
        return None;
    }
    let title_tokens = tokenize(candidate_title);
    let uploader_tokens = tokenize(uploader);
    let mut hits = 0usize;
    for artist in artists {
        let artist_tokens = tokenize(artist);
        if artist_tokens.is_empty() {
            continue;
        }
        let found = artist_tokens
            .iter()
            .all(|token| title_tokens.contains(token) || uploader_tokens.contains(token));
        if found {
            hits += 1;
        }
    }
    Some(hits as f64 / artists.len() as f64)
}

fn duration_similarity(candidate_secs: Option<f64>, expected_secs: Option<u64>) -> Option<f64> {
    let expected = expected_secs? as f64;
    let Some(candidate) = candidate_secs else {
        return Some(0.5);
    };
    let diff = (candidate - expected).abs();
    let score = if diff <= 2.0 {
        1.0
    } else if diff <= 5.0 {
        0.8
    } else if diff <= 10.0 {
        0.5
    } else if diff <= 20.0 {
        0.2
    } else {
        0.0
    };
    Some(score)
}

fn core_tokens(input: &str, removed_artists: &[&str]) -> Vec<String> {
    let mut without_phrases = normalize_text(input);
    for phrase in NOISE_PHRASES {
        without_phrases = without_phrases.replace(phrase, " ");
    }
    let mut tokens: Vec<String> = without_phrases
        .split_whitespace()
        .map(str::to_string)
        .filter(|token| !is_noise_token(token))
        .collect();
    for artist in removed_artists {
        let artist_tokens = tokenize(artist);
        if artist_tokens.is_empty() {
            continue;
        }
        tokens.retain(|token| !artist_tokens.contains(token));
    }
    tokens
}

fn tokenize(input: &str) -> Vec<String> {
    normalize_text(input)
        .split_whitespace()
        .map(str::to_string)
        .collect()
}

pub fn normalize_text(input: &str) -> String {
    let mut normalized = String::with_capacity(input.len());
    for character in input.chars() {
        if character.is_ascii_alphanumeric() {
            normalized.push(character.to_ascii_lowercase());
        } else if character.is_alphanumeric() {
            for lowered in character.to_lowercase() {
                if lowered.is_alphanumeric() {
                    normalized.push(fold_char(lowered));
                }
            }
        } else {
            normalized.push(' ');
        }
    }
    normalized
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
}

fn fold_char(character: char) -> char {
    match character {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' | 'ạ' | 'ả' | 'ấ' | 'ầ' | 'ẩ' | 'ẫ'
        | 'ậ' | 'ắ' | 'ằ' | 'ẳ' | 'ẵ' | 'ặ' => 'a',
        'ḃ' | 'ḅ' | 'ḇ' => 'b',
        'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' | 'ḉ' => 'c',
        'ď' | 'ḋ' | 'ḍ' | 'ḏ' | 'ḑ' | 'ḓ' | 'đ' | 'ð' => 'd',
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' | 'ḕ' | 'ḗ' | 'ḙ' | 'ḛ' | 'ḝ' | 'ẹ'
        | 'ẻ' | 'ẽ' | 'ế' | 'ề' | 'ể' | 'ễ' | 'ệ' => 'e',
        'ḟ' => 'f',
        'ĝ' | 'ğ' | 'ġ' | 'ģ' | 'ḡ' => 'g',
        'ĥ' | 'ḣ' | 'ḥ' | 'ḧ' | 'ḩ' | 'ḫ' | 'ħ' | 'ẖ' => 'h',
        'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ḭ' | 'ḯ' | 'ỉ' | 'ị' => {
            'i'
        }
        'ĵ' => 'j',
        'ķ' | 'ḱ' | 'ḳ' | 'ḵ' => 'k',
        'ĺ' | 'ļ' | 'ľ' | 'ḷ' | 'ḹ' | 'ḻ' | 'ḽ' | 'ł' => 'l',
        'ḿ' | 'ṁ' | 'ṃ' => 'm',
        'ñ' | 'ń' | 'ņ' | 'ň' | 'ṅ' | 'ṇ' | 'ṉ' | 'ṋ' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ō' | 'ŏ' | 'ő' | 'ø' | 'ơ' | 'ọ' | 'ỏ' | 'ố' | 'ồ' | 'ổ'
        | 'ỗ' | 'ộ' | 'ớ' | 'ờ' | 'ở' | 'ỡ' | 'ợ' => 'o',
        'ṕ' | 'ṗ' => 'p',
        'ŕ' | 'ŗ' | 'ř' | 'ṙ' | 'ṛ' | 'ṝ' | 'ṟ' => 'r',
        'ś' | 'ŝ' | 'ş' | 'š' | 'ṡ' | 'ṣ' | 'ṥ' | 'ṧ' | 'ṩ' => 's',
        'ţ' | 'ť' | 'ṫ' | 'ṭ' | 'ṯ' | 'ṱ' | 'ŧ' | 'ẗ' => 't',
        'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' | 'ư' | 'ụ' | 'ủ' | 'ứ' | 'ừ'
        | 'ử' | 'ữ' | 'ự' => 'u',
        'ṽ' | 'ṿ' => 'v',
        'ŵ' | 'ẁ' | 'ẃ' | 'ẅ' | 'ẇ' | 'ẉ' | 'ẘ' => 'w',
        'ẋ' | 'ẍ' => 'x',
        'ý' | 'ÿ' | 'ŷ' | 'ẏ' | 'ỳ' | 'ỵ' | 'ỷ' | 'ỹ' | 'ẙ' => 'y',
        'ź' | 'ż' | 'ž' | 'ẑ' | 'ẓ' | 'ẕ' => 'z',
        other => other,
    }
}

fn dice_bigrams(left: &str, right: &str) -> f64 {
    let left_bigrams = bigrams(left);
    let right_bigrams = bigrams(right);
    if left_bigrams.is_empty() || right_bigrams.is_empty() {
        return if left == right && !left.is_empty() {
            1.0
        } else {
            0.0
        };
    }
    let mut matches = 0usize;
    let mut remaining = right_bigrams.clone();
    for bigram in &left_bigrams {
        if let Some(position) = remaining.iter().position(|candidate| candidate == bigram) {
            remaining.remove(position);
            matches += 1;
        }
    }
    2.0 * matches as f64 / (left_bigrams.len() + right_bigrams.len()) as f64
}

fn bigrams(text: &str) -> Vec<String> {
    let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    let characters: Vec<char> = compact.chars().collect();
    if characters.len() < 2 {
        return Vec::new();
    }
    characters
        .windows(2)
        .map(|pair| pair.iter().collect::<String>())
        .collect()
}

fn is_noise_token(token: &str) -> bool {
    NOISE_TOKENS.contains(&token)
}

fn is_placeholder_artist(artist: &str) -> bool {
    let normalized = normalize_text(artist);
    normalized.is_empty() || PLACEHOLDER_ARTISTS.contains(&normalized.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(title: &str, uploader: &str, duration: Option<f64>) -> SearchCandidate {
        SearchCandidate {
            url: format!("https://www.youtube.com/watch?v={title}"),
            title: title.to_string(),
            uploader: uploader.to_string(),
            duration_secs: duration,
        }
    }

    #[test]
    fn normalize_text_folds_accents_and_diacritics() {
        assert_eq!(normalize_text("Nắng Thơ"), "nang tho");
        assert_eq!(normalize_text("Đâu Ai Đáng"), "dau ai dang");
        assert_eq!(normalize_text("Êm Đềm"), "em dem");
        assert_eq!(normalize_text("Hòa Bình"), "hoa binh");
        assert_eq!(normalize_text("À-Á!"), "a a");
    }

    #[test]
    fn title_similarity_ignores_noise_tags() {
        let artists: Vec<&str> = Vec::new();
        let score = title_similarity("Nắng Thơ (Official Music Video)", "Nắng Thơ", &artists);
        assert!(score > 0.95, "score {score}");
    }

    #[test]
    fn title_similarity_ignores_artist_prefix() {
        let artists = vec!["Hoàng Dũng"];
        let score = title_similarity(
            "Hoàng Dũng - Thấy Lòng Bao La (Official MV)",
            "Thấy Lòng Bao La",
            &artists,
        );
        assert!(score > 0.9, "score {score}");
    }

    #[test]
    fn title_similarity_handles_missing_diacritics() {
        let artists: Vec<&str> = Vec::new();
        let score = title_similarity("Nang Tho - Fan Upload", "Nắng Thơ", &artists);
        assert!(score >= 0.85, "score {score}");
    }

    #[test]
    fn title_similarity_keeps_music_when_part_of_real_title() {
        let artists: Vec<&str> = Vec::new();
        let score = title_similarity("Music Box (Official Video)", "Music Box", &artists);
        assert!(score > 0.95, "score {score}");
    }

    #[test]
    fn pick_best_prefers_exact_match_over_remix() {
        let candidates = vec![
            candidate("Bài Này Chill Phet (Remix)", "DJ Someone", Some(200.0)),
            candidate(
                "Bài Này Chill Phet (Official MV)",
                "Phí Phương Anh",
                Some(201.0),
            ),
        ];
        let artists = vec!["Phí Phương Anh".to_string()];
        let best = pick_best(&candidates, "Bài Này Chill Phet", &artists, Some(201)).unwrap();
        assert_eq!(best.uploader, "Phí Phương Anh");
    }

    #[test]
    fn pick_best_rejects_unrelated_result() {
        let candidates = vec![candidate(
            "Livestream Highlights compilation",
            "Random Channel",
            Some(3599.0),
        )];
        let artists = vec!["Sơn Tùng M-TP".to_string()];
        assert!(pick_best(&candidates, "Hãy Trao Cho Anh", &artists, Some(235)).is_none());
    }

    #[test]
    fn pick_best_accepts_duration_close_candidate_when_title_matches() {
        let candidates = vec![candidate(
            "Song Name Official Audio",
            "Artist Name - Topic",
            Some(183.0),
        )];
        let artists = vec!["Artist Name".to_string()];
        let best = pick_best(&candidates, "Song Name", &artists, Some(180)).unwrap();
        assert_eq!(
            best.url,
            "https://www.youtube.com/watch?v=Song Name Official Audio"
        );
    }

    #[test]
    fn pick_best_uses_duration_to_break_title_ties() {
        let candidates = vec![
            candidate("Song Name", "Channel A", Some(300.0)),
            candidate("Song Name", "Channel B", Some(181.0)),
        ];
        let artists: Vec<String> = Vec::new();
        let best = pick_best(&candidates, "Song Name", &artists, Some(180)).unwrap();
        assert_eq!(best.uploader, "Channel B");
    }

    #[test]
    fn pick_best_treats_placeholder_artists_as_unknown() {
        let candidates = vec![candidate(
            "Query Words Official Audio",
            "Some Channel",
            None,
        )];
        let artists = vec!["Search".to_string()];
        assert!(
            pick_best(&candidates, "Query Words", &artists, None).is_some(),
            "placeholder artist should not block a matching candidate"
        );
    }

    #[test]
    fn dice_bigrams_handles_single_characters() {
        assert!((dice_bigrams("a", "a") - 1.0).abs() < 1e-9);
        assert!(dice_bigrams("a", "b").abs() < 1e-9);
        assert!(dice_bigrams("", "abc").abs() < 1e-9);
    }
}
