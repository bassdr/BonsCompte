<?php
  include_once('./sql.php');

  function print_table($title, $mysqli, $sql,
    $header_name, $sql_name, $facultative = NULL)
  {
    $sql_result = $mysqli->query($sql);
    if(!$sql_result)
      die($mysqli->error);
    if($sql_result->num_rows == 0)
    {
      echo("\n<!--Query: La table '$title' est vide-->\n");
    }
    else
    {
      echo("\n<h2>$title</h2>\n");
      echo("<table>\n");
      echo("<tr>\n");
      foreach($header_name as $i)
      {
        echo("<th>$i</th>\n");
      }
      echo("</tr>\n");
      while($row = $sql_result->fetch_array(MYSQLI_ASSOC))
      {
        echo("<tr>\n");
        foreach($sql_name as $i)
        {
          echo("<td>");
          if(!empty($row[$i]))
          {
            echo("$row[$i]");
          }
          if(!empty($facultative[$i]) && !empty($row[$facultative[$i]]))
          {
            echo(" (" . $row[$facultative[$i]] . ")");
          }
          echo("</td>\n");
        }
        echo("</tr>\n");
      }
      echo("</table>\n");
    }
    $sql_result->close();
  }

  function print_option($name, $value, $mysqli, $first, $sql,
    $sql_id, $sql_name, $facultative = NULL, $disabled)
  {
    $sql_result = $mysqli->query($sql);
    if(!$sql_result)
    {
      die($mysqli->error);
    }

    if($sql_result->num_rows == 0)
    {
      echo("\n<!--Query: Le menu déroulant '$name' des vide-->\n");
    }
    else
    {
      if($disabled)
      {
        //workaround: disables the selection so the user cannot change
        //the value, but still post the value in a hidden input.
        echo("<input type='hidden' name='$name' value='$value' />\n");
        echo("<select disabled='true'>\n");
      }
      else
      {
        echo("<select name='$name'>\n");
      }
      echo("<option value='NULL'>$first</option>\n");
      while($row = $sql_result->fetch_array(MYSQLI_ASSOC))
      {
        echo("<option value='$row[$sql_id]'");
        if($value == $row[$sql_id])
        {
          echo(" selected='true'");
        }
        echo(">");
        echo("$row[$sql_name]");
        if(!empty($row[$facultative]))
        {
          echo(" (" . $row[$facultative]. ")");
        }
        echo("</option>\n");
      }
      echo("</select>\n");
    }
    $sql_result->close();
  }

  function get_name($name, $mysqli)
  {
    $sql_result = $mysqli->query("SELECT fname FROM user WHERE uname = '$name'");
    if(!$sql_result)
    {
      die($mysqli->error);
    }

    while($row = $sql_result->fetch_array(MYSQLI_ASSOC))
    {
      $full_name = "$row[fname]";
    }

    $sql_result->close();
    return $full_name;
  }

  function print_name($name, $mysqli)
  {
    echo(get_name("$name", $mysqli));
  }
?>
