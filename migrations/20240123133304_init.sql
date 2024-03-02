CREATE TABLE `auth` (
  `id` int(9) unsigned NOT NULL AUTO_INCREMENT,
  `access_token` varchar(255) NOT NULL,
  `expiry_date` INT(11) NULL ,
  `refresh_token` varchar(255) NOT NULL,
  PRIMARY KEY (`id`)
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb3 COLLATE = utf8mb3_general_ci;

CREATE TABLE `user` (
  `username` varchar(255) NOT NULL,
  `auth_id` int(9) unsigned NOT NULL,
  PRIMARY KEY (`username`),
  CONSTRAINT `user_ibfk_1` FOREIGN KEY (`auth_id`) REFERENCES `auth` (`id`) ON DELETE CASCADE
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb3 COLLATE = utf8mb3_general_ci;