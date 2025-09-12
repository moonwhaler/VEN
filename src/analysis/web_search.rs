use crate::config::ContentType;
use crate::utils::Result;
use reqwest::Client;
use std::time::Duration;
use regex::Regex;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use tracing::{info, debug};

#[derive(Debug, Clone)]
pub struct TitleExtraction {
    pub title: String,
    pub confidence: f32,
    pub method: String,
}

#[derive(Debug, Clone)]
pub struct WebSearchResult {
    pub content_type: ContentType,
    pub confidence: f32,
    pub source: String,
}

pub struct WebSearchClassifier {
    client: Client,
    timeout: Duration,
    enabled: bool,
    simulation_mode: bool,
    content_database: HashMap<String, ContentType>,
}

impl WebSearchClassifier {
    pub fn new(timeout_seconds: u64, enabled: bool, simulation_mode: bool) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_seconds))
            .user_agent("FFmpeg-Autoencoder/3.0")
            .build()
            .unwrap();

        let mut content_database = HashMap::new();
        // Populate simulation database with known content
        content_database.insert("spirited away".to_string(), ContentType::Anime);
        content_database.insert("princess mononoke".to_string(), ContentType::Anime);
        content_database.insert("akira".to_string(), ContentType::ClassicAnime);
        content_database.insert("ghost in the shell".to_string(), ContentType::ClassicAnime);
        content_database.insert("toy story".to_string(), ContentType::Animation3D);
        content_database.insert("incredibles".to_string(), ContentType::Animation3D);
        content_database.insert("arcane".to_string(), ContentType::Animation3D);
        content_database.insert("blade runner 2049".to_string(), ContentType::HeavyGrain);
        content_database.insert("dune".to_string(), ContentType::HeavyGrain);
        content_database.insert("mad max fury road".to_string(), ContentType::Action);
        content_database.insert("john wick".to_string(), ContentType::Action);

        Self {
            client,
            timeout: Duration::from_secs(timeout_seconds),
            enabled,
            simulation_mode,
            content_database,
        }
    }

    pub async fn classify_from_filename(&self, filename: &str) -> Result<Option<WebSearchResult>> {
        if !self.enabled {
            return Ok(None);
        }

        // Extract title from filename
        let title_extraction = self.extract_title_from_filename(filename);
        if title_extraction.title.is_empty() {
            return Ok(None);
        }

        // Perform content classification
        if self.simulation_mode {
            self.simulate_web_search(&title_extraction.title).await
        } else {
            self.perform_web_search(&title_extraction.title).await
        }
    }

    pub fn extract_title_from_filename(&self, filename: &str) -> TitleExtraction {
        // TV Show Pattern: Title.S01E01
        static TV_SHOW_REGEX: Lazy<Regex> = Lazy::new(|| {
            Regex::new(r"^(.+?)\.S\d{2}E\d{2}").unwrap()
        });

        // Movie with Year: Title.2024
        static MOVIE_YEAR_REGEX: Lazy<Regex> = Lazy::new(|| {
            Regex::new(r"^(.+?)\.(19|20)\d{2}").unwrap()
        });

        // Quality indicators to remove
        static QUALITY_REGEX: Lazy<Regex> = Lazy::new(|| {
            Regex::new(r"\.(720p|1080p|4K|2160p|x264|x265|H\.?264|H\.?265|HEVC|BluRay|WEB-DL|HDRip|BRRip)").unwrap()
        });

        let filename_clean = filename.to_lowercase();

        // Try TV show pattern first
        if let Some(captures) = TV_SHOW_REGEX.captures(&filename_clean) {
            let title = captures.get(1).map_or("", |m| m.as_str());
            return TitleExtraction {
                title: self.clean_title(title),
                confidence: 0.9, // High confidence for TV show pattern
                method: "tv_show_pattern".to_string(),
            };
        }

        // Try movie with year pattern
        if let Some(captures) = MOVIE_YEAR_REGEX.captures(&filename_clean) {
            let title = captures.get(1).map_or("", |m| m.as_str());
            return TitleExtraction {
                title: self.clean_title(title),
                confidence: 0.85, // High confidence for movie year pattern
                method: "movie_year_pattern".to_string(),
            };
        }

        // Remove quality indicators and try general pattern
        let cleaned = QUALITY_REGEX.replace_all(&filename_clean, "");
        let title = cleaned.split('.').next().unwrap_or("").trim();

        if !title.is_empty() {
            TitleExtraction {
                title: self.clean_title(title),
                confidence: 0.6, // Lower confidence for general extraction
                method: "filename_extraction".to_string(),
            }
        } else {
            TitleExtraction {
                title: String::new(),
                confidence: 0.0,
                method: "failed".to_string(),
            }
        }
    }

    fn clean_title(&self, title: &str) -> String {
        title.replace(['.', '_', '-'], " ")
            .trim()
            .to_string()
    }

    async fn simulate_web_search(&self, title: &str) -> Result<Option<WebSearchResult>> {
        let title_lower = title.to_lowercase();
        
        // Check built-in database
        for (known_title, content_type) in &self.content_database {
            if title_lower.contains(known_title) || known_title.contains(&title_lower) {
                return Ok(Some(WebSearchResult {
                    content_type: *content_type,
                    confidence: 0.8,
                    source: "simulation_database".to_string(),
                }));
            }
        }

        // Keyword-based classification as fallback
        let classification = self.classify_by_keywords(&title_lower);
        Ok(Some(classification))
    }

    async fn perform_web_search(&self, title: &str) -> Result<Option<WebSearchResult>> {
        // Generate search queries for different sources
        let queries = vec![
            format!("{} anime", title),
            format!("{} movie", title),
            format!("{} film", title),
            format!("{} animation", title),
        ];

        let mut best_result: Option<WebSearchResult> = None;
        let mut best_confidence = 0.0;

        for query in queries {
            match self.search_query(&query).await {
                Ok(Some(result)) if result.confidence > best_confidence => {
                    best_confidence = result.confidence;
                    best_result = Some(result);
                }
                _ => continue,
            }
        }

        if best_result.is_none() {
            // Fallback to keyword-based classification
            let classification = self.classify_by_keywords(&title.to_lowercase());
            Ok(Some(classification))
        } else {
            Ok(best_result)
        }
    }

    async fn search_query(&self, query: &str) -> Result<Option<WebSearchResult>> {
        info!("Performing real web search for query: {}", query);
        
        // Try TMDB API first (The Movie Database)
        if let Ok(Some(result)) = self.search_tmdb_api(query).await {
            return Ok(Some(result));
        }
        
        // Fallback to OMDB API
        if let Ok(Some(result)) = self.search_omdb_api(query).await {
            return Ok(Some(result));
        }
        
        // Final fallback - generic web search (placeholder for now)
        self.search_generic_web(query).await
    }

    async fn search_tmdb_api(&self, query: &str) -> Result<Option<WebSearchResult>> {
        // TMDB API endpoint (using demo key for now - would need real API key)
        let search_url = format!(
            "https://api.themoviedb.org/3/search/multi?api_key=demo&query={}",
            urlencoding::encode(query)
        );
        
        match self.client
            .get(&search_url)
            .timeout(self.timeout)
            .send()
            .await 
        {
            Ok(response) if response.status().is_success() => {
                if let Ok(json) = response.json::<serde_json::Value>().await {
                    return Ok(self.parse_tmdb_response(&json, query));
                }
            }
            Ok(response) => {
                debug!("TMDB API returned status: {}", response.status());
            }
            Err(e) => {
                debug!("TMDB API request failed: {}", e);
            }
        }
        
        Ok(None)
    }

    async fn search_omdb_api(&self, query: &str) -> Result<Option<WebSearchResult>> {
        // OMDB API endpoint (using demo key - would need real API key)
        let search_url = format!(
            "http://www.omdbapi.com/?apikey=demo&t={}&plot=short",
            urlencoding::encode(query)
        );
        
        match self.client
            .get(&search_url)
            .timeout(self.timeout)
            .send()
            .await 
        {
            Ok(response) if response.status().is_success() => {
                if let Ok(json) = response.json::<serde_json::Value>().await {
                    return Ok(self.parse_omdb_response(&json, query));
                }
            }
            Ok(response) => {
                debug!("OMDB API returned status: {}", response.status());
            }
            Err(e) => {
                debug!("OMDB API request failed: {}", e);
            }
        }
        
        Ok(None)
    }

    async fn search_generic_web(&self, query: &str) -> Result<Option<WebSearchResult>> {
        // Generic web search using DuckDuckGo Instant Answer API (no API key needed)
        let search_url = format!(
            "https://api.duckduckgo.com/?q={}&format=json&no_redirect=1",
            urlencoding::encode(&format!("{} movie", query))
        );
        
        match self.client
            .get(&search_url)
            .timeout(self.timeout)
            .send()
            .await 
        {
            Ok(response) if response.status().is_success() => {
                if let Ok(json) = response.json::<serde_json::Value>().await {
                    return Ok(self.parse_duckduckgo_response(&json, query));
                }
            }
            Ok(response) => {
                debug!("DuckDuckGo API returned status: {}", response.status());
            }
            Err(e) => {
                debug!("DuckDuckGo API request failed: {}", e);
            }
        }
        
        Ok(None)
    }

    fn parse_tmdb_response(&self, json: &serde_json::Value, _query: &str) -> Option<WebSearchResult> {
        if let Some(results) = json["results"].as_array() {
            if let Some(first_result) = results.first() {
                let media_type = first_result["media_type"].as_str().unwrap_or("unknown");
                let empty_vec = vec![];
                let genre_ids = first_result["genre_ids"].as_array().unwrap_or(&empty_vec);
                let overview = first_result["overview"].as_str().unwrap_or("");
                
                let content_type = self.classify_from_tmdb_data(media_type, genre_ids, overview);
                
                return Some(WebSearchResult {
                    content_type,
                    confidence: 0.85,
                    source: "TMDB API".to_string(),
                });
            }
        }
        
        None
    }

    fn parse_omdb_response(&self, json: &serde_json::Value, _query: &str) -> Option<WebSearchResult> {
        if json["Response"].as_str() == Some("True") {
            let genre = json["Genre"].as_str().unwrap_or("");
            let plot = json["Plot"].as_str().unwrap_or("");
            let media_type = json["Type"].as_str().unwrap_or("movie");
            
            let content_type = self.classify_from_omdb_data(genre, plot, media_type);
            
            return Some(WebSearchResult {
                content_type,
                confidence: 0.80,
                source: "OMDB API".to_string(),
            });
        }
        
        None
    }

    fn parse_duckduckgo_response(&self, json: &serde_json::Value, query: &str) -> Option<WebSearchResult> {
        // Check abstract for content hints
        if let Some(abstract_text) = json["Abstract"].as_str() {
            if !abstract_text.is_empty() {
                let content_type = self.classify_from_text_analysis(abstract_text, query);
                return Some(WebSearchResult {
                    content_type,
                    confidence: 0.60, // Lower confidence for generic web search
                    source: "DuckDuckGo API".to_string(),
                });
            }
        }
        
        None
    }

    fn classify_from_tmdb_data(&self, media_type: &str, genre_ids: &[serde_json::Value], overview: &str) -> ContentType {
        // TMDB genre IDs: 16=Animation, 28=Action, 18=Drama, 35=Comedy, etc.
        let animation_genres = [16]; // Animation
        let action_genres = [28, 53, 80]; // Action, Thriller, Crime
        
        for genre_id in genre_ids {
            if let Some(id) = genre_id.as_u64() {
                if animation_genres.contains(&(id as u32)) {
                    // Further classify animation
                    if overview.to_lowercase().contains("anime") {
                        return ContentType::Anime;
                    } else if overview.contains("3D") || overview.contains("Pixar") || overview.contains("Disney") {
                        return ContentType::Animation3D;
                    } else {
                        return ContentType::ClassicAnime;
                    }
                } else if action_genres.contains(&(id as u32)) {
                    return ContentType::Action;
                }
            }
        }
        
        // Default classification based on media type
        match media_type {
            "tv" => ContentType::Mixed,
            _ => ContentType::Film,
        }
    }

    fn classify_from_omdb_data(&self, genre: &str, plot: &str, media_type: &str) -> ContentType {
        let genre_lower = genre.to_lowercase();
        let plot_lower = plot.to_lowercase();
        
        // Check for animation first
        if genre_lower.contains("animation") {
            if plot_lower.contains("anime") || genre_lower.contains("anime") {
                return ContentType::Anime;
            } else if plot_lower.contains("3d") || plot_lower.contains("pixar") || plot_lower.contains("disney") {
                return ContentType::Animation3D;
            } else {
                return ContentType::ClassicAnime;
            }
        }
        
        // Check for action content
        if genre_lower.contains("action") || genre_lower.contains("thriller") {
            return ContentType::Action;
        }
        
        // Check for heavy grain content indicators
        if plot_lower.contains("noir") || plot_lower.contains("gritty") || 
           plot_lower.contains("dystopian") || genre_lower.contains("thriller") {
            return ContentType::HeavyGrain;
        }
        
        // Default based on media type
        match media_type {
            "series" => ContentType::Mixed,
            _ => ContentType::Film,
        }
    }

    fn classify_from_text_analysis(&self, text: &str, query: &str) -> ContentType {
        let text_lower = text.to_lowercase();
        let query_lower = query.to_lowercase();
        
        // Animation indicators
        if text_lower.contains("animated") || text_lower.contains("animation") {
            if text_lower.contains("anime") || query_lower.contains("anime") {
                return ContentType::Anime;
            } else if text_lower.contains("3d") || text_lower.contains("pixar") {
                return ContentType::Animation3D;
            } else {
                return ContentType::ClassicAnime;
            }
        }
        
        // Action indicators
        if text_lower.contains("action") || text_lower.contains("thriller") || 
           text_lower.contains("adventure") {
            return ContentType::Action;
        }
        
        // Heavy grain indicators
        if text_lower.contains("noir") || text_lower.contains("gritty") || 
           text_lower.contains("dark") {
            return ContentType::HeavyGrain;
        }
        
        // Default
        ContentType::Film
    }

    fn classify_by_keywords(&self, text: &str) -> WebSearchResult {
        let text_lower = text.to_lowercase();
        
        // Anime keywords (highest priority)
        let anime_keywords = ["anime", "manga", "studio ghibli", "miyazaki", "makoto shinkai", 
                             "evangelion", "naruto", "dragon ball", "one piece", "attack on titan"];
        let anime_score = anime_keywords.iter()
            .map(|&keyword| if text_lower.contains(keyword) { 3.0 } else { 0.0 })
            .sum::<f32>();

        // Classic anime keywords
        let classic_anime_keywords = ["akira", "ghost in the shell", "cowboy bebop", "90s anime", 
                                     "80s anime", "cel animation"];
        let classic_anime_score = classic_anime_keywords.iter()
            .map(|&keyword| if text_lower.contains(keyword) { 3.0 } else { 0.0 })
            .sum::<f32>();

        // 3D Animation keywords
        let animation_3d_keywords = ["pixar", "dreamworks", "disney animation", "3d animation", 
                                   "cgi", "toy story", "incredibles", "shrek", "frozen", "arcane"];
        let animation_3d_score = animation_3d_keywords.iter()
            .map(|&keyword| if text_lower.contains(keyword) { 3.0 } else { 0.0 })
            .sum::<f32>();

        // Heavy grain keywords
        let heavy_grain_keywords = ["film noir", "70s", "grain", "vintage", "blade runner", 
                                   "mad max", "alien", "terminator"];
        let heavy_grain_score = heavy_grain_keywords.iter()
            .map(|&keyword| if text_lower.contains(keyword) { 2.0 } else { 0.0 })
            .sum::<f32>();

        // Action keywords
        let action_keywords = ["action", "thriller", "chase", "fight", "explosion", "superhero",
                              "marvel", "dc comics", "fast and furious", "mission impossible"];
        let action_score = action_keywords.iter()
            .map(|&keyword| if text_lower.contains(keyword) { 2.0 } else { 0.0 })
            .sum::<f32>();

        // Light grain keywords
        let light_grain_keywords = ["documentary", "clean", "digital", "modern film"];
        let light_grain_score = light_grain_keywords.iter()
            .map(|&keyword| if text_lower.contains(keyword) { 1.5 } else { 0.0 })
            .sum::<f32>();

        // Determine the highest scoring category
        let scores = vec![
            (ContentType::Anime, anime_score, "anime_keywords"),
            (ContentType::ClassicAnime, classic_anime_score, "classic_anime_keywords"),
            (ContentType::Animation3D, animation_3d_score, "3d_animation_keywords"),
            (ContentType::HeavyGrain, heavy_grain_score, "heavy_grain_keywords"),
            (ContentType::Action, action_score, "action_keywords"),
            (ContentType::LightGrain, light_grain_score, "light_grain_keywords"),
        ];

        let (content_type, score, source) = scores.into_iter()
            .max_by(|(_, a, _), (_, b, _)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or((ContentType::Film, 0.0, "default"));

        let confidence = if score > 0.0 {
            (score / 10.0).min(0.9) // Normalize score to confidence
        } else {
            0.3 // Default confidence for film
        };

        WebSearchResult {
            content_type: if score > 0.0 { content_type } else { ContentType::Film },
            confidence,
            source: source.to_string(),
        }
    }

    pub async fn enhance_classification_with_web_search(
        &self,
        technical_classification: ContentType,
        technical_confidence: f32,
        filename: &str,
    ) -> Result<(ContentType, f32, String)> {
        if !self.enabled {
            return Ok((technical_classification, technical_confidence, "technical_only".to_string()));
        }

        match self.classify_from_filename(filename).await? {
            Some(web_result) => {
                // Combine technical and web search results
                let combined_confidence = (technical_confidence + web_result.confidence) / 2.0;
                
                // If web search confidence is significantly higher, prefer web result
                if web_result.confidence > technical_confidence + 0.2 {
                    Ok((web_result.content_type, web_result.confidence, 
                       format!("web_search_preferred_{}", web_result.source)))
                } else if technical_confidence > web_result.confidence + 0.2 {
                    Ok((technical_classification, technical_confidence, "technical_preferred".to_string()))
                } else {
                    // Use web search result but with combined confidence
                    Ok((web_result.content_type, combined_confidence, 
                       format!("web_technical_combined_{}", web_result.source)))
                }
            }
            None => Ok((technical_classification, technical_confidence, "technical_fallback".to_string()))
        }
    }
}

impl Default for WebSearchClassifier {
    fn default() -> Self {
        Self::new(10, true, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_title_extraction_tv_show() {
        let classifier = WebSearchClassifier::default();
        let extraction = classifier.extract_title_from_filename("Game.of.Thrones.S01E01.720p.mkv");
        
        assert_eq!(extraction.title, "game of thrones");
        assert_eq!(extraction.method, "tv_show_pattern");
        assert!(extraction.confidence > 0.8);
    }

    #[test]
    fn test_title_extraction_movie_year() {
        let classifier = WebSearchClassifier::default();
        let extraction = classifier.extract_title_from_filename("Blade.Runner.2049.1080p.BluRay.x265.mkv");
        
        assert_eq!(extraction.title, "blade runner");
        assert_eq!(extraction.method, "movie_year_pattern");
        assert!(extraction.confidence > 0.8);
    }

    #[test]
    fn test_keyword_classification() {
        let classifier = WebSearchClassifier::default();
        let result = classifier.classify_by_keywords("spirited away anime studio ghibli");
        
        assert_eq!(result.content_type, ContentType::Anime);
        assert!(result.confidence > 0.5);
    }

    #[test]
    fn test_simulation_database() {
        let mut classifier = WebSearchClassifier::default();
        classifier.simulation_mode = true;
        
        tokio_test::block_on(async {
            let result = classifier.classify_from_filename("Spirited.Away.2001.1080p.mkv").await.unwrap();
            
            assert!(result.is_some());
            let result = result.unwrap();
            assert_eq!(result.content_type, ContentType::Anime);
        });
    }

    #[test]
    fn test_quality_indicator_removal() {
        let classifier = WebSearchClassifier::default();
        let extraction = classifier.extract_title_from_filename("Movie.Title.2023.2160p.4K.HDR.x265.HEVC.mkv");
        
        assert_eq!(extraction.title, "movie title");
        // Should not contain quality indicators
        assert!(!extraction.title.contains("2160p"));
        assert!(!extraction.title.contains("x265"));
    }
}

// Helper function for URL encoding
mod urlencoding {
    pub fn encode(input: &str) -> String {
        input.chars().map(|c| {
            match c {
                'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
                ' ' => "+".to_string(),
                _ => format!("%{:02X}", c as u8),
            }
        }).collect()
    }
}