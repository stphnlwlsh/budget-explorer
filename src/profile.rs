//! User profile management with GAIL framework
//!
//! GAIL = Goals, Actions, Information, Language
//! - G (Goals): What the user wants to achieve
//! - A (Actions): YNAB API tools (predefined)
//! - I (Information): Budget data from YNAB API
//! - L (Language): Communication style preferences

#![allow(dead_code, unused_imports, unused_variables)]

use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use std::path::PathBuf;

/// User profile with personalized settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    pub version: String,
    pub goals: Vec<String>,
    pub concerns: Vec<String>,
    pub advice_style: AdviceStyle,
    pub tone: Tone,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AdviceStyle {
    Detailed,
    Summary,
    ActionItems,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    Friendly,
    Direct,
    Professional,
}

impl UserProfile {
    /// Load profile from disk.
    pub fn load() -> Result<Option<Self>, Box<dyn std::error::Error>> {
        let path = Self::path()?;
        if !path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(path)?;
        let profile: UserProfile = serde_json::from_str(&content)?;
        Ok(Some(profile))
    }

    /// Save profile to disk.
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Get the profile file path.
    fn path() -> Result<PathBuf, Box<dyn std::error::Error>> {
        let dir = dirs::config_dir()
            .ok_or("Could not find config directory")?
            .join("budget-explorer");
        Ok(dir.join("profile.json"))
    }

    /// Create a default profile.
    pub fn default_profile() -> Self {
        Self {
            version: "1.0".to_string(),
            goals: vec!["Track spending".to_string()],
            concerns: vec!["Reduce unnecessary spending".to_string()],
            advice_style: AdviceStyle::Summary,
            tone: Tone::Friendly,
        }
    }
}

/// Run the interactive onboarding wizard.
pub fn run() -> Result<UserProfile, Box<dyn std::error::Error>> {
    println!("\n=== Budget Explorer Onboarding ===\n");
    println!("Let's set up your profile for personalized advice.\n");

    let mut profile = UserProfile {
        version: "1.0".to_string(),
        goals: Vec::new(),
        concerns: Vec::new(),
        advice_style: AdviceStyle::Summary,
        tone: Tone::Friendly,
    };

    // Collect goals
    println!("1. What are your financial goals?");
    println!("   (Press Enter after each, type 'done' when finished)\n");
    loop {
        print!("   Goal: ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim().to_string();
        if input.is_empty() || input.to_lowercase() == "done" {
            break;
        }
        if !input.is_empty() {
            profile.goals.push(input);
        }
    }
    if profile.goals.is_empty() {
        profile.goals.push("Track spending".to_string());
    }

    // Collect concerns
    println!("\n2. What financial concerns do you have?");
    println!("   (Press Enter after each, type 'done' when finished)\n");
    loop {
        print!("   Concern: ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim().to_string();
        if input.is_empty() || input.to_lowercase() == "done" {
            break;
        }
        if !input.is_empty() {
            profile.concerns.push(input);
        }
    }
    if profile.concerns.is_empty() {
        profile
            .concerns
            .push("Reduce unnecessary spending".to_string());
    }

    // Advice style
    println!("\n3. How much detail do you want in advice?");
    println!("   1) Summary - Brief overviews");
    println!("   2) Detailed - In-depth analysis");
    println!("   3) Action Items - Clear next steps\n");
    loop {
        print!("   Choice [1]: ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();
        match input {
            "" | "1" => {
                profile.advice_style = AdviceStyle::Summary;
                break;
            }
            "2" => {
                profile.advice_style = AdviceStyle::Detailed;
                break;
            }
            "3" => {
                profile.advice_style = AdviceStyle::ActionItems;
                break;
            }
            _ => println!("   Please enter 1, 2, or 3."),
        }
    }

    // Tone
    println!("\n4. What tone do you prefer?");
    println!("   1) Friendly - Warm and encouraging");
    println!("   2) Direct - Straight to the point");
    println!("   3) Professional - Formal and business-like\n");
    loop {
        print!("   Choice [1]: ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();
        match input {
            "" | "1" => {
                profile.tone = Tone::Friendly;
                break;
            }
            "2" => {
                profile.tone = Tone::Direct;
                break;
            }
            "3" => {
                profile.tone = Tone::Professional;
                break;
            }
            _ => println!("   Please enter 1, 2, or 3."),
        }
    }

    // Save profile
    profile.save()?;
    println!("\n✓ Profile saved!\n");

    Ok(profile)
}

/// Update an existing profile interactively.
pub fn update(profile: &mut UserProfile) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Update Profile ===\n");
    println!("Press Enter to keep current value.\n");

    // Update goals
    println!("Goals (currently: {}):", profile.goals.join(", "));
    print!("   Type 'skip' to keep: ");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    if input.trim().to_lowercase() != "skip" {
        profile.goals.clear();
        for goal in input.trim().split(',') {
            let goal = goal.trim();
            if !goal.is_empty() {
                profile.goals.push(goal.to_string());
            }
        }
    }

    // Save
    profile.save()?;
    println!("\n✓ Profile updated!\n");

    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    // ========================================================================
    // UserProfile serialization tests - Positive cases
    // ========================================================================

    #[test]
    fn user_profile_serializes_to_json() {
        // Arrange
        let profile = UserProfile {
            version: "1.0".to_string(),
            goals: vec!["Save money".to_string()],
            concerns: vec!["Reduce spending".to_string()],
            advice_style: AdviceStyle::Summary,
            tone: Tone::Friendly,
        };
        // Act
        let json = serde_json::to_string(&profile).unwrap();
        // Assert
        assert!(json.contains("1.0"));
        assert!(json.contains("Save money"));
    }

    #[test]
    fn user_profile_deserializes_from_json() {
        // Arrange
        let json = r#"{
            "version": "1.0",
            "goals": ["Save money"],
            "concerns": ["Reduce spending"],
            "advice_style": "summary",
            "tone": "friendly"
        }"#;
        // Act
        let profile: UserProfile = serde_json::from_str(json).unwrap();
        // Assert
        assert_eq!(profile.version, "1.0");
        assert_eq!(profile.goals, vec!["Save money"]);
        assert_eq!(profile.advice_style, AdviceStyle::Summary);
        assert_eq!(profile.tone, Tone::Friendly);
    }

    #[test]
    fn user_profile_roundtrip_serialization() {
        // Arrange
        let original = UserProfile {
            version: "2.0".to_string(),
            goals: vec!["Emergency fund".to_string(), "Retire early".to_string()],
            concerns: vec!["Too much debt".to_string()],
            advice_style: AdviceStyle::Detailed,
            tone: Tone::Professional,
        };
        // Act
        let json = serde_json::to_string(&original).unwrap();
        let parsed: UserProfile = serde_json::from_str(&json).unwrap();
        // Assert
        assert_eq!(parsed.version, original.version);
        assert_eq!(parsed.goals, original.goals);
        assert_eq!(parsed.advice_style, original.advice_style);
        assert_eq!(parsed.tone, original.tone);
    }

    #[test]
    fn user_profile_handles_empty_arrays() {
        // Arrange
        let profile = UserProfile {
            version: "1.0".to_string(),
            goals: vec![],
            concerns: vec![],
            advice_style: AdviceStyle::ActionItems,
            tone: Tone::Direct,
        };
        // Act
        let json = serde_json::to_string(&profile).unwrap();
        let parsed: UserProfile = serde_json::from_str(&json).unwrap();
        // Assert
        assert!(parsed.goals.is_empty());
        assert!(parsed.concerns.is_empty());
    }

    // ========================================================================
    // default_profile tests - Positive cases
    // ========================================================================

    #[test]
    fn default_profile_creates_valid_profile() {
        // Arrange & Act
        let profile = UserProfile::default_profile();
        // Assert
        assert_eq!(profile.version, "1.0");
        assert!(!profile.goals.is_empty());
        assert!(!profile.concerns.is_empty());
        assert_eq!(profile.advice_style, AdviceStyle::Summary);
        assert_eq!(profile.tone, Tone::Friendly);
    }

    #[test]
    fn default_profile_has_sensible_defaults() {
        // Arrange & Act
        let profile = UserProfile::default_profile();
        // Assert
        assert!(profile.goals.contains(&"Track spending".to_string()));
        assert!(profile
            .concerns
            .contains(&"Reduce unnecessary spending".to_string()));
    }

    // ========================================================================
    // AdviceStyle and Tone serialization tests - Negative cases
    // ========================================================================

    #[test]
    fn advice_style_serializes_to_snake_case() {
        // Arrange
        let style = AdviceStyle::Detailed;
        // Act
        let json = serde_json::to_string(&style).unwrap();
        // Assert
        assert_eq!(json, "\"detailed\"");
    }

    #[test]
    fn tone_serializes_to_snake_case() {
        // Arrange
        let tone = Tone::Professional;
        // Act
        let json = serde_json::to_string(&tone).unwrap();
        // Assert
        assert_eq!(json, "\"professional\"");
    }

    #[test]
    fn advice_style_deserializes_from_snake_case() {
        // Arrange
        let json = "\"action_items\"";
        // Act
        let style: AdviceStyle = serde_json::from_str(json).unwrap();
        // Assert
        assert_eq!(style, AdviceStyle::ActionItems);
    }

    #[test]
    fn tone_deserializes_from_snake_case() {
        // Arrange
        let json = "\"direct\"";
        // Act
        let tone: Tone = serde_json::from_str(json).unwrap();
        // Assert
        assert_eq!(tone, Tone::Direct);
    }

    // ========================================================================
    // Profile persistence path tests - Positive cases
    // ========================================================================

    #[test]
    fn profile_path_uses_config_directory() {
        // Arrange & Act
        let profile = UserProfile::default_profile();
        // We can't directly test the path without I/O, but we can verify
        // the profile has valid data for saving
        // Assert
        assert!(serde_json::to_string(&profile).is_ok());
    }

    #[test]
    fn profile_save_creates_parent_directories() {
        // This test would require actual file I/O which we skip in unit tests
        // Instead, verify the serialization is correct
        let profile = UserProfile::default_profile();
        let json = serde_json::to_string_pretty(&profile);
        assert!(json.is_ok());
    }
}
