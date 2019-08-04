<?php
  session_start();
  header("Content-Type: text/html; charset=utf-8");
  include_once("sql.php");
  $nodebt = db_connect();

  if(!empty($_POST["do_insert"]))
  {
    if(empty($_SESSION["passwd"]) || $_SESSION["passwd"] != "Zaq12wsx")
    {
      die("Illegal attemt to query database without appropriate rights");
    }
    $total_money = get_number("calc_total_money");
    $date = get_date("calc_date");
    $description = get_string("description");
    $user = get_string("user");
    $receiver = get_string("receiver");
    $contrib_num = get_number("calc_contrib_num");
    if($contrib_num > 0)
    {
      for($i = 0; $i < $contrib_num; $i++)
      {
        $contrib[$i] = get_string("calc_contrib$i");
        $money[$i] = get_number("calc_money$i");
      }
      $sql = "
        INSERT INTO payment(user, receiver, date, description )
        VALUES (
          (SELECT id FROM user WHERE uname = '$user'),
          (SELECT id FROM user WHERE uname = '$receiver'),
          '$date',
          '$description'
        )
      ";
      $sql_result = $nodebt->query($sql);
      if(!$sql_result)
      {
        die($nodebt->error . ": " . $sql);
      }
      else
      {
        $payment_id = $nodebt->insert_id;
        $sql = "INSERT INTO contribution(user, payment, contribution)
        VALUES ";
        for($i = 0; $i < $contrib_num; $i++)
        {
          if($i != 0)
            $sql .= ", ";
          $sql .= "(
            (SELECT id FROM user WHERE uname = '$contrib[$i]'),
            $payment_id,
            $money[$i]
          )";
        }
        $sql .= "ON DUPLICATE KEY
          UPDATE contribution = VALUES(contribution) + contribution";

        $sql_result = $nodebt->query($sql);
        if(!$sql_result)
        {
          $sql_error = $nodebt->error;
          $nodebt->query("DELETE FROM payment WHERE id='$payment_id'");
          die($sql_error . ": " . $sql);
        }
        else
        {
          $raw_user = get_raw_string('user');
          if($raw_user == 'NULL')
            $raw_user = get_raw_string('receiver');

          header("Location: " . strtok("$_SERVER[REQUEST_URI]", '?') . "/../show.php?user=$raw_user");
        }

      }

    }
    else
    {
      die("La transaction doit contenir au moins une personne!");
    }

  }
  else
  {
    $check_insert = !empty($_REQUEST["check_insert"]);
    $total_money = get_raw_string("total_money");
    $date = get_raw_string("date");
    $description = get_raw_string("description");

    $user = get_raw_string("user");
    $receiver = get_raw_string("receiver");
    $fault = false;
    if($receiver == 'NULL' && $user == 'NULL')
    {
      $fault = true;
    }

    $contrib_num = get_raw_string("contrib_num");
    if($contrib_num == NULL)
    {
      $contrib_num = 3;
    }

    $money_format = get_raw_string("money_format");

    for($i = 0; $i < db_number($contrib_num); $i++)
    {
      $contrib[$i] = get_raw_string("contrib$i");
      $money[$i] = get_raw_string("money$i");
    }

    if(!empty($_SESSION["passwd"]))
    {
      $passwd = $_SESSION["passwd"];
    }
    else
    {
      $passwd = get_raw_string("passwd");
      if($passwd == "Zaq12wsx")
      {
        $_SESSION["passwd"] = $passwd;
      }
    }

    if($check_insert)
    {
      $calc_total_money = db_number($total_money);
      $calc_date = db_date($date);
      $calc_contrib_num = db_number($contrib_num);
      $sum_money = 0;
      $split_money_in = 0;
      for($index = 0, $i = 0; $index < $calc_contrib_num && $i < $contrib_num; $index++, $i++)
      {
        if($contrib[$i] == 'NULL')
        {
          $calc_contrib_num--;
          $index--;
          continue;
        }
        $calc_contrib[$index] = $contrib[$i];

        $nb_parts[$index] = 1;

        if($money_format == '\$')
        {
          $calc_money[$index] = db_number($money[$i]);
        }
        elseif($money_format == '%')
        {
          if($calc_total_money == 0)
          {
            $fault = true;
          }
          $calc_money[$index] = db_number($money[$i])/100.0*$calc_total_money;
        }
        elseif($money_format == 'parts')
        {
          if($calc_total_money == 0)
          {
            $fault = true;
          }
          $nb_parts[$index] = db_number($money[$i]);
          if($nb_parts[$index] == 0)
          {
            $nb_parts[$index] = 1;
          }
          $split_money_in += $nb_parts[$index];
        }

        if(!isset($calc_money[$index]))
        {
          $calc_money[$index] = 0;
        }

        if($money_format != 'parts')
        {
          if($calc_money[$index] == 0)
          {
            $split_money_in++;
          }
          $sum_money += $calc_money[$index];
        }
        else
        {
          $sum_money = 0;
        }
      }

      if($calc_total_money != $sum_money)
      {
        if($calc_total_money < $sum_money)
        {
          $calc_total_money = $sum_money;
        }
        elseif($calc_total_money > $sum_money && $split_money_in > 0)
        {
          $splitted_money = ($calc_total_money - $sum_money)/$split_money_in;
          for($i = 0; $i < $calc_contrib_num; $i++)
          {
            if($calc_money[$i] == 0)
            {
              $calc_money[$i] = $splitted_money * $nb_parts[$i];
            }
          }
        }
        elseif($calc_total_money > $sum_money && $split_money_in == 0)
        {
          $calc_contrib_num++;
          if($user == 'NULL')
          {
            $calc_contrib[$index] = $receiver;
          }
          else
          {
            $calc_contrib[$index] = $user;
          }
          $calc_money[$index] = $calc_total_money - $sum_money;
        }
      }
      if($calc_contrib_num <= 0)
      {
        $fault = true;
      }
      if(empty($_SESSION["passwd"]))
      {
        $fault = true;
      }
    }

    $total_money = db_string($total_money);
    $date = db_string($date);
    $description = db_string($description);
    $contrib_num = db_string($contrib_num);
    for($i = 0; $i < $contrib_num; $i++)
    {
      $contrib[$i] = db_string($contrib[$i]);
      $money[$i] = db_string($money[$i]);
    }
?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xml:lang="fr" lang="fr">
<head>
<title>Ajout d'une transaction &ndash; Bons comptes</title>
<meta http-equiv="Content-Type" content="text/html; charset=utf-8" />
<style>
body
{
  background-image: url("images/billets.jpg");
  background-position: center top;
}

p
{
  background-color:#eeeeff;
  opacity:1;
  filter:alpha(opacity=100); /* For IE8 and earlier */
}

h1
{
  background-color:#fefeff;
  opacity:1;
  filter:alpha(opacity=100); /* For IE8 and earlier */
}

h2
{
  background-color:#eeeeff;
  opacity:0.7;
  filter:alpha(opacity=70); /* For IE8 and earlier */
}
</style>
</head>
<body>
<?php
    include_once("functions.php");
    echo("<h1>Ajouter une transaction</h1>\n");

    //clear the parameters in the url, so they are not sent twice.
    echo("<form method='post' action='".strtok("$_SERVER[REQUEST_URI]", '?')
       . "?user=$_REQUEST[user]'>\n");

    //needed if we cancel the action
    if($check_insert)
    {
      if($fault)
      {
        if($receiver == 'NULL' && $user == 'NULL')
        {
          echo("<p style='color:#DF2D2D'>\n<b>Erreur&nbsp;: Il faut au moins qu'un ");
          echo("utilisateur fasse la transaction ou la reçoive pour que ");
          echo("celle-ci soit valide&nbsp;!</b>\n</p>\n");
        }
        if($calc_contrib_num <= 0)
        {
          echo("<p style='color:#DF2D2D'>\n<b>Erreur&nbsp;: Il faut au moins qu'un ");
          echo("utilisateur contribu à la transaction pour que ");
          echo("celle-ci soit valide&nbsp;!</b>\n</p>\n");
        }
        if($calc_total_money == 0 && ($money_format == '%' || $money_format == 'parts'))
        {
          echo("<p style='color:#DF2D2D'>\n<b>Erreur&nbsp;: Le montant de la transation ");
          echo("est obligatoire lorsque l'on veut séparer la transaction ");
          echo("en parties inégales&nbsp;!</b>\n</p>\n");
        }
        if(empty($_SESSION["passwd"]))
        {
          echo("<p style='color:#DF2D2D'>\n<b>Erreur&nbsp;: Mauvais mot de passe.</b>\n</p>\n");
        }
        echo("<p style='color:#DF2D2D'>\n<b>Une erreur est survenue</b>\n</p>\n");
      }
      echo("<input type='hidden' name='total_money' value='$total_money' />\n");
      echo("<input type='hidden' name='date' value='$date' />\n");
      echo("<input type='hidden' name='contrib_num' value='$contrib_num' />\n");
      for($i = 0; $i < $contrib_num; $i++)
      {
        echo("<input type='hidden' name='contrib$i' value='$contrib[$i]' />\n");
        echo("<input type='hidden' name='money$i' value='$money[$i]' />\n");
      }
    }

    echo("<p>Montant de la transaction&nbsp;: ");
    echo("<input type='text' max_length='45' ");
    if($check_insert)
      echo("name='calc_total_money' value='$calc_total_money' readonly='true' ");
    else
      echo("name='total_money' value='$total_money' ");
    echo("/>&nbsp;\$</p>\n");

    echo("<p>Date&nbsp;: ");
    echo("<input type='text' maxlength='40' ");
    if($check_insert)
      echo("name='calc_date' value='$calc_date' readonly='true' ");
    else
      echo("name='date' value='$date' ");
    echo("/>&nbsp;(aaaa-mm-jj [hh:mm:ss]), vide pour MAINTENANT</p>\n");

    echo("<p>Description&nbsp;: ");
    //TODO: ajouter cette fonction dès que du text est écrit en html
    echo("<input type='text' name='description' value='$description' ");
    echo("maxlength='1000' style='width:550px' ");
    if($check_insert)
      echo("readonly='true' ");
    echo("/></p>\n");

    echo("<p>Personne qui effectue la transaction&nbsp;: \n");
    print_option("user", $user, $nodebt,
      "Pas un utilisateur de Bons Comptes", "
        SELECT fname, uname FROM user
      ",
      "uname", "fname", "uname", $check_insert && !$fault);
    echo("</p>\n");

    echo("<p>Personne qui recevera l'argent de la transaction&nbsp;: \n");
    print_option("receiver", $receiver, $nodebt,
      "Pas un utilisateur de Bons Comptes", "
        SELECT fname, uname FROM user
      ",
      "uname", "fname", "uname", $check_insert && !$fault);
    echo("</p>\n");

    echo("<p>Il y a ");
    echo("<input type='text' maxlength='2' style='width:25px' ");
    if($check_insert)
      echo("name='calc_contrib_num' value='$calc_contrib_num' readonly='true' ");
    else
      echo("name='contrib_num' value='$contrib_num' ");
    echo("/>");
    echo(" personnes qui doivent fournir.\n");
    echo("<input type='submit' value='changer' ");
    if($check_insert)
      echo("disabled='true' ");
    echo("/>");
    echo("</p>\n");

    if(!$check_insert || $calc_contrib_num > 0)
    {
      echo("<p>Ces personnes sont&nbsp;:</p>\n");
    }

    echo("<table>\n");

    if(!$check_insert)
    {
      echo("<tr>\n<td>\n</td>\n<td>\n");
      echo("<select name='money_format'>\n");
      echo("<option value='\$'");
      if($money_format == '\$')
      {
        echo(" selected='true'");
      }
      echo(">Montant en argent (\$)</option>\n");
      echo("<option value='%'");
      if($money_format == '%')
      {
        echo(" selected='true'");
      }
      echo(">Pourcentage de séparation (%)</option>\n");
      echo("<option value='parts'");
      if($money_format == 'parts')
      {
        echo(" selected='true'");
      }
      echo(">Parts dans la transaction</option>\n");
      echo("</select>\n");
    }
    else
    {
      echo("<tr>\n<td>\n");
      //post the value in a hidden input.
      echo("<input type='hidden' name='money_format' value='$money_format' />\n");
      if($calc_contrib_num > 0)
      {
        echo("</td>\n<td>\n<b>\$</b>\n</td>\n<td>\n<b>%</b>\n");
      }
    }
    echo("</td>\n</tr>\n");

    for($i = 0; (!$check_insert && $i < $contrib_num) || ($check_insert && $i < $calc_contrib_num); $i++)
    {
      echo("<tr>\n<td>\n");
      if($check_insert)
        $calc = "calc_";
      else
        $calc = "";
      if((!$check_insert && empty($contrib[$i])) || ($check_insert && empty($calc_contrib[$i])))
        $val = "";
      elseif($check_insert)
        $val = $calc_contrib[$i];
      else
        $val = $contrib[$i];
      print_option("${calc}contrib$i", $val, $nodebt, "", "
          SELECT fname, uname FROM user
        ",
        "uname", "fname", "uname", $check_insert);

      if(!$check_insert && empty($money[$i]))
        $val = "";
      elseif($check_insert)
        $val = $calc_money[$i];
      else
        $val = $money[$i];
      echo("</td>\n<td>\n<input type='text' name='${calc}money$i' value='$val' ");
      echo("max_length='23' ");
      if($check_insert)
      {
        echo("readonly='true' />\n</td>\n");
        echo("<td>\n<input type='text' name='${calc}percent$i' value='" . $val*100.0/$calc_total_money . "' readonly='true' ");
      }
      echo("/>\n</td>\n</tr>\n");
    }
    echo("</table>\n");

    echo("<p>\n");
    if(!$check_insert || $fault)
    {
      if(empty($_SESSION["passwd"]))
      {
        echo("Password: <input type='password' name='passwd' ");
        if($fault)
        {
          echo("disabled='true' value='wrong, wrong, and wrong!' ");
        }

        echo("/>\n");
      }

      echo("<input type='submit' name='check_insert' value='Ajouter la transaction' ");
      if($fault)
      {
        echo("disabled='true' ");
      }
      echo("/>\n");
    }
    else
    {
      echo("<input type='submit' name='do_insert' value='Confirmer' ");
      if($fault)
      {
        echo("disabled='true' ");
      }
      echo("/>\n");
    }

    if($check_insert || $fault)
    {
      echo("<input type='submit' name='cancel' value='Annuler' />\n");
    }
    echo("</p>\n");

    echo("</form>\n");
?>
</body>
</html>
<?php
  }
  $nodebt->close();
?>
